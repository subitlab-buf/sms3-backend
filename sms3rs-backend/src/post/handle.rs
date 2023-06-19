use std::{
    hash::{Hash, Hasher},
    ops::Deref,
};

use sms3rs_shared::post::handle::*;

/// Read and store a cache image with cache id returned.
///
/// Url: `/api/post/upload-image`
///
/// Request header: See [`crate::RequirePermissionContext`].
///
/// Request body: Image bytes.
///
/// Response body: `200` with `{ "hash": _ }`. (json)
#[actix_web::post("/api/post/upload-image")]
pub async fn cache_image(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
    data: actix_web::web::Bytes,
) -> impl actix_web::Responder {
    match cxt
        .valid(vec![sms3rs_shared::account::Permission::Post])
        .await
    {
        Ok(able) => {
            if able {
                let id;
                super::cache::INSTANCE
                    .push(
                        match crate::post::cache::PostImageCache::new(
                            &{
                                if data.len() > 50_000_000 {
                                    return (
                                        String::new(),
                                        actix_web::http::StatusCode::PAYLOAD_TOO_LARGE,
                                    );
                                }
                                data
                            },
                            cxt.account_id,
                        ) {
                            Ok(e) => {
                                id = e.1;
                                e.0
                            }
                            Err(err) => {
                                return (
                                    err.to_string(),
                                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                                )
                            }
                        },
                    )
                    .await;
                (
                    serde_json::to_string(&serde_json::json!({ "hash": id })).unwrap(),
                    actix_web::http::StatusCode::OK,
                )
            } else {
                (
                    "Permission denied".to_string(),
                    actix_web::http::StatusCode::FORBIDDEN,
                )
            }
        }
        Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
    }
}

/// Get image png bytes from target image cache hash.
///
/// Url: `/api/post/get-image/?hash={image_hash}`
///
/// Request header: See [`crate::RequirePermissionContext`].
///
/// Response body: `200` with image bytes.
#[actix_web::get("/api/post/get-image")]
pub async fn get_image(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
    actix_web::web::Query(ImageHashTarget { hash }): actix_web::web::Query<ImageHashTarget>,
) -> impl actix_web::Responder {
    match cxt
        .valid(vec![sms3rs_shared::account::Permission::View])
        .await
    {
        Ok(able) => {
            if able {
                for img in super::cache::INSTANCE.caches.read().await.iter() {
                    if img.hash == hash {
                        #[cfg(test)]
                        return (Vec::new(), actix_web::http::StatusCode::NO_CONTENT);

                        #[cfg(not(test))]
                        {
                            use tokio::io::AsyncReadExt;

                            match tokio::fs::File::open(format!("./data/images/{}.png", img.hash))
                                .await
                            {
                                Ok(mut file) => {
                                    let mut vec = Vec::new();
                                    let _ = file.read_to_end(&mut vec).await;
                                    return (vec, actix_web::http::StatusCode::OK);
                                }
                                Err(_) => {
                                    return (
                                        Vec::new(),
                                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                                    )
                                }
                            }
                        }
                    }
                }

                (Vec::new(), actix_web::http::StatusCode::NOT_FOUND)
            } else {
                (Vec::new(), actix_web::http::StatusCode::FORBIDDEN)
            }
        }
        Err(_) => (Vec::new(), actix_web::http::StatusCode::UNAUTHORIZED),
    }
}

/// Handle and create a new pending post.
///
/// Url: `/api/post/create`
///
/// Request header: See [`crate::RequirePermissionContext`].
///
/// Request body: See [`PostDescriptor`]
///
/// Response body: `200` with `{ "post_id": _ }`. (json)
#[actix_web::post("/api/post/create")]
pub async fn create_post(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
    descriptor: actix_web::web::Json<CreatePostDescriptor>,
) -> impl actix_web::Responder {
    match cxt
        .valid(vec![sms3rs_shared::account::Permission::Post])
        .await
    {
        Ok(able) => {
            if able {
                let cache = super::cache::INSTANCE.caches.read().await;
                for img_id in descriptor.images.iter() {
                    if !cache.iter().any(|e| e.hash == *img_id) {
                        return (
                            format!("Image cache with hash {img_id} not found"),
                            actix_web::http::StatusCode::NOT_FOUND,
                        );
                    }
                }

                for img_id in descriptor.images.iter() {
                    cache
                        .iter()
                        .find(|e| e.hash == *img_id)
                        .unwrap()
                        .blocked
                        .store(true, std::sync::atomic::Ordering::Relaxed)
                }

                let post = sms3rs_shared::post::Post {
                    id: {
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        descriptor.title.hash(&mut hasher);
                        descriptor.description.hash(&mut hasher);
                        descriptor.images.hash(&mut hasher);
                        let id = hasher.finish();
                        if super::INSTANCE.contains_id(id).await {
                            return (
                                "Post id conflicted".to_string(),
                                actix_web::http::StatusCode::CONFLICT,
                            );
                        }
                        id
                    },
                    images: descriptor.images.iter().copied().collect(),
                    status: {
                        let mut deque = std::collections::VecDeque::new();
                        deque.push_back(super::PostAcceptationData {
                            operator: cxt.account_id,
                            status: super::PostAcceptationStatus::Pending,
                            time: chrono::Utc::now(),
                        });
                        deque
                    },
                    metadata: super::PostMetadata {
                        title: descriptor.title.clone(),
                        description: descriptor.description.clone(),
                        time_range: {
                            if descriptor.time_range.0 + chrono::Days::new(7)
                                < descriptor.time_range.1
                            {
                                return (
                                    "Post time out of range".to_string(),
                                    actix_web::http::StatusCode::FORBIDDEN,
                                );
                            }
                            descriptor.time_range
                        },
                    },
                    publisher: cxt.account_id,
                };

                if !super::save_post(&post).await {
                    tracing::error!("Error while saving post {}", post.id);
                }

                let id = post.id;
                super::INSTANCE.push(post).await;

                (
                    serde_json::to_string(&serde_json::json!({ "post_id": id })).unwrap(),
                    actix_web::http::StatusCode::OK,
                )
            } else {
                (
                    "Permission denied".to_string(),
                    actix_web::http::StatusCode::FORBIDDEN,
                )
            }
        }
        Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
    }
}

/// Get posts based on the given filters.
///
/// Url: `/api/post/get`
///
/// Request header: See [`crate::RequirePermissionContext`].
///
/// Request body: See [`GetPostsDescriptor`].
///
/// Response body: `200` with `{ "posts": [_] }`. (json)
#[actix_web::post("/api/post/get")]
pub async fn get_posts(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
    descriptor: actix_web::web::Json<GetPostsDescriptor>,
) -> impl actix_web::Responder {
    match cxt
        .valid(vec![sms3rs_shared::account::Permission::View])
        .await
    {
        Ok(able) => {
            if able {
                let am = crate::account::INSTANCE.inner().read().await;
                let ar = am
                    .get(
                        *crate::account::INSTANCE
                            .index()
                            .read()
                            .await
                            .get(&cxt.account_id)
                            .unwrap(),
                    )
                    .unwrap()
                    .read()
                    .await;

                let mut posts = Vec::new();

                for p in super::INSTANCE.posts.read().await.iter() {
                    let pr = p.read().await;
                    if descriptor
                        .filters
                        .iter()
                        .all(|f| matches_get_post_filter(f, pr.deref(), ar.deref()))
                    {
                        posts.push(pr.id);
                    }
                }

                (
                    serde_json::to_string(&serde_json::json!({ "posts": posts })).unwrap(),
                    actix_web::http::StatusCode::OK,
                )
            } else {
                (
                    "Permission denied".to_string(),
                    actix_web::http::StatusCode::FORBIDDEN,
                )
            }
        }
        Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
    }
}

/// Whether the target post matches this filter and
/// the target account has enough permission to get the post.
///
/// Not a request handling method.
fn matches_get_post_filter(
    filter: &GetPostsFilter,
    post: &sms3rs_shared::post::Post,
    account: &crate::account::Account,
) -> bool {
    let date = chrono::Utc::now().date_naive();
    (match filter {
        GetPostsFilter::Acceptation(status) => post
            .status
            .back()
            .map_or(false, |s| status.matches(&s.status)),
        GetPostsFilter::Account(account) => &post.publisher == account,
        GetPostsFilter::Before(d) => &post.metadata.time_range.0 <= d,
        GetPostsFilter::After(d) => &post.metadata.time_range.0 >= d,
        GetPostsFilter::Keyword(keywords) => {
            let ks = keywords.split_whitespace();
            ks.into_iter()
                .all(|k| post.metadata.title.contains(k) && post.metadata.description.contains(k))
        }
    }) && (post.publisher == account.id()
        || (post.metadata.time_range.0 <= date
            && account.has_permission(sms3rs_shared::account::Permission::View))
        || account.has_permission(sms3rs_shared::account::Permission::Check))
}

/// Edit a post.
///
/// Url: `/api/post/edit`
///
/// Request header: See [`crate::RequirePermissionContext`].
///
/// Request body: See [`EditPostDescriptor`].
#[actix_web::post("/api/post/edit")]
pub async fn edit_post(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
    descriptor: actix_web::web::Json<EditPostDescriptor>,
) -> impl actix_web::Responder {
    match cxt
        .valid(vec![sms3rs_shared::account::Permission::Post])
        .await
    {
        Ok(able) => {
            if able {
                for p in super::INSTANCE.posts.read().await.iter() {
                    let pr = p.read().await;
                    if pr.id == descriptor.post {
                        if pr.publisher != cxt.account_id
                            || pr
                                .status
                                .back()
                                .map(|e| {
                                    matches!(
                                        e.status,
                                        sms3rs_shared::post::PostAcceptationStatus::Submitted(_)
                                    )
                                })
                                .unwrap_or_default()
                        {
                            return (
                                "Permission denied".to_string(),
                                actix_web::http::StatusCode::FORBIDDEN,
                            );
                        }
                        let id = pr.id;
                        drop(pr);
                        for variant in descriptor.variants.iter() {
                            match apply_edit_post_variant(variant, id, cxt.account_id).await {
                                Err(err) => {
                                    return (
                                        format!("Unable to edit post: {err}"),
                                        actix_web::http::StatusCode::FORBIDDEN,
                                    );
                                }
                                _ => (),
                            }
                        }
                        let post = p.read().await;
                        if !super::save_post(post.deref()).await {
                            tracing::error!("Error while saving post {}", post.id);
                        }

                        return (String::new(), actix_web::http::StatusCode::OK);
                    }
                }

                (
                    "Target post not found".to_string(),
                    actix_web::http::StatusCode::NOT_FOUND,
                )
            } else {
                (
                    "Permission denied".to_string(),
                    actix_web::http::StatusCode::FORBIDDEN,
                )
            }
        }
        Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
    }
}

/// Apply this edition and return an err if error occurs.
/// Not a request handling method.
pub async fn apply_edit_post_variant(
    variant: &EditPostVariant,
    post_id: u64,
    user_id: u64,
) -> anyhow::Result<()> {
    let posts = super::INSTANCE.posts.read().await;
    let mut post = match {
        let mut e = None;
        for p in posts.iter() {
            if p.read().await.id == post_id {
                e = Some(p.write().await);
                break;
            }
        }
        e
    } {
        Some(e) => e,
        None => return Err(anyhow::anyhow!("Post not found")),
    };
    match variant {
        EditPostVariant::Title(value) => post.metadata.title = value.clone(),
        EditPostVariant::Description(value) => post.metadata.description = value.clone(),
        EditPostVariant::Images(imgs) => {
            let cache = super::cache::INSTANCE.caches.read().await;
            for img_id in post.images.iter() {
                if let Some(e) = cache.iter().find(|e| e.hash == *img_id) {
                    e.blocked.store(false, std::sync::atomic::Ordering::Relaxed)
                }
            }
            for img_id in imgs.iter() {
                if !cache.iter().any(|e| e.hash == *img_id) {
                    return Err(anyhow::anyhow!("Target image cache {} not fount", img_id));
                }
            }
            for img_id in imgs.iter() {
                let mut unlock = true;
                for e in super::INSTANCE.posts.read().await.iter() {
                    if let Ok(er) = e.try_read() {
                        if er.images.contains(img_id) {
                            unlock = false;
                            break;
                        }
                    }
                }

                if unlock {
                    cache
                        .iter()
                        .find(|e| e.hash == *img_id)
                        .unwrap()
                        .blocked
                        .store(true, std::sync::atomic::Ordering::Relaxed)
                }
            }
        }
        EditPostVariant::TimeRange(start, end) => {
            if *start + chrono::Days::new(7) < *end {
                return Err(anyhow::anyhow!("Post time out of range"));
            }
            post.metadata.time_range = (*start, *end);
        }
        EditPostVariant::CancelSubmission => {
            if post.status.back().map_or(true, |e| {
                matches!(
                    e.status,
                    sms3rs_shared::post::PostAcceptationStatus::Pending
                )
            }) {
                return Err(anyhow::anyhow!("Target post was already pended"));
            }
            post.status.push_back(super::PostAcceptationData {
                operator: user_id,
                status: sms3rs_shared::post::PostAcceptationStatus::Pending,
                time: chrono::Utc::now(),
            })
        }
        EditPostVariant::Destroy => {
            if !super::remove_post(post.deref()).await {
                tracing::error!("Post {} save failed", post.id);
            }
            drop(post);
            drop(posts);
            let mut posts = super::INSTANCE.posts.write().await;
            let mut i = None;
            for post in posts.iter().enumerate() {
                let pr = post.1.read().await;
                if pr.id == post_id {
                    i = Some(post.0);
                    for img_id in pr.images.iter() {
                        let mut unlock = true;
                        for e in super::INSTANCE.posts.read().await.iter() {
                            if let Ok(er) = e.try_read() {
                                if er.images.contains(img_id) {
                                    unlock = false;
                                    break;
                                }
                            }
                        }

                        if unlock {
                            for im in super::cache::INSTANCE.caches.read().await.iter() {
                                if &im.hash == img_id {
                                    im.blocked
                                        .store(false, std::sync::atomic::Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                    }
                    break;
                }
            }
            match i {
                Some(e) => {
                    posts.remove(e);
                }
                None => return Err(anyhow::anyhow!("Post not found")),
            }
        }
        EditPostVariant::RequestReview(msg) => {
            if post.status.back().map_or(true, |e| {
                matches!(
                    e.status,
                    sms3rs_shared::post::PostAcceptationStatus::Submitted(_)
                )
            }) {
                return Err(anyhow::anyhow!("Target post was already submitted"));
            }
            post.status.push_back(super::PostAcceptationData {
                operator: user_id,
                status: sms3rs_shared::post::PostAcceptationStatus::Submitted(msg.clone()),
                time: chrono::Utc::now(),
            })
        }
    }
    Ok(())
}

/// Get posts info.
///
/// Url: `/api/post/get-info`
///
/// Request header: See [`crate::RequirePermissionContext`].
///
/// Request body: See [`GetPostsInfoDescriptor`].
///
/// Response body: `200` with `{ "results": [_] }`,
/// see [`GetPostInfoResult`]. (json)
#[actix_web::post("/api/post/get-info")]
pub async fn get_posts_info(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
    descriptor: actix_web::web::Json<GetPostsInfoDescriptor>,
) -> impl actix_web::Responder {
    match cxt
        .valid(vec![sms3rs_shared::account::Permission::View])
        .await
    {
        Ok(able) => {
            if able {
                let am = crate::account::INSTANCE.inner().read().await;
                let ar = am
                    .get(
                        *crate::account::INSTANCE
                            .index()
                            .read()
                            .await
                            .get(&cxt.account_id)
                            .unwrap(),
                    )
                    .unwrap()
                    .read()
                    .await;
                let mut results = Vec::new();
                let posts = super::INSTANCE.posts.read().await;
                let date = chrono::Utc::now().date_naive();
                for p in descriptor.posts.iter() {
                    let mut ps = false;
                    for e in posts.iter() {
                        let er = e.read().await;
                        if er.id == *p {
                            if er.publisher == cxt.account_id
                                || ar.has_permission(sms3rs_shared::account::Permission::Check)
                            {
                                results.push(GetPostInfoResult::Full(er.clone()))
                            } else if er.metadata.time_range.0 <= date
                                && ar.has_permission(sms3rs_shared::account::Permission::View)
                            {
                                results.push(GetPostInfoResult::Foreign {
                                    id: er.id,
                                    images: er.images.clone(),
                                    title: er.metadata.title.clone(),
                                    archived: er.metadata.time_range.1 < date,
                                })
                            } else {
                                results.push(GetPostInfoResult::NotFound(er.id))
                            }
                            ps = true;
                            break;
                        }
                    }
                    if !ps {
                        results.push(GetPostInfoResult::NotFound(*p))
                    }
                }

                (
                    serde_json::to_string(&serde_json::json!({ "results": results })).unwrap(),
                    actix_web::http::StatusCode::OK,
                )
            } else {
                (
                    "Permission denied".to_string(),
                    actix_web::http::StatusCode::FORBIDDEN,
                )
            }
        }
        Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
    }
}

/// Accept or reject a post.
///
/// Url: `/api/post/approve`
///
/// Request header: See [`crate::RequirePermissionContext`].
///
/// Request body: See [`ApprovePostDescriptor`].
#[actix_web::post("/api/post/approve")]
pub async fn approve_post(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
    descriptor: actix_web::web::Json<ApprovePostDescriptor>,
) -> impl actix_web::Responder {
    match cxt
        .valid(vec![sms3rs_shared::account::Permission::Approve])
        .await
    {
        Ok(able) => {
            if able {
                let posts = super::INSTANCE.posts.read().await;
                for p in posts.iter() {
                    let pr = p.read().await;
                    if pr.id == descriptor.post {
                        drop(pr);
                        let mut pw = p.write().await;
                        match descriptor.into_inner().variant {
                            ApprovePostVariant::Accept(msg) => {
                                if pw.status.back().map_or(false, |e| {
                                    matches!(
                                        e.status,
                                        sms3rs_shared::post::PostAcceptationStatus::Accepted(_)
                                    )
                                }) {
                                    return (
                                        "Target post has already been accepted".to_string(),
                                        actix_web::http::StatusCode::FORBIDDEN,
                                    );
                                }
                                pw.status.push_back(super::PostAcceptationData {
                                    operator: cxt.account_id,
                                    status: sms3rs_shared::post::PostAcceptationStatus::Accepted(
                                        msg.unwrap_or_default(),
                                    ),
                                    time: chrono::Utc::now(),
                                });
                            }
                            ApprovePostVariant::Reject(msg) => {
                                if pw.status.back().map_or(false, |e| {
                                    matches!(
                                        e.status,
                                        sms3rs_shared::post::PostAcceptationStatus::Rejected(_)
                                    )
                                }) {
                                    return (
                                        "Target post has already been rejected".to_string(),
                                        actix_web::http::StatusCode::FORBIDDEN,
                                    );
                                }
                                pw.status.push_back(super::PostAcceptationData {
                                    operator: cxt.account_id,
                                    status: sms3rs_shared::post::PostAcceptationStatus::Rejected({
                                        if msg.is_empty() {
                                            return (
                                                "Message couldn't be empty".to_string(),
                                                actix_web::http::StatusCode::FORBIDDEN,
                                            );
                                        }
                                        msg
                                    }),
                                    time: chrono::Utc::now(),
                                });
                            }
                        };

                        if !super::save_post(pw.deref()).await {
                            tracing::error!("Error while saving post {}", pw.id);
                        }

                        return (String::new(), actix_web::http::StatusCode::OK);
                    }
                }

                (
                    "Target post not found".to_string(),
                    actix_web::http::StatusCode::NOT_FOUND,
                )
            } else {
                (
                    "Permission denied".to_string(),
                    actix_web::http::StatusCode::FORBIDDEN,
                )
            }
        }
        Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
    }
}
