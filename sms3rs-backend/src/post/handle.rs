use super::Post;
use super::PostAcceptationStatus;
use crate::account;
use crate::account::Account;
use crate::account::Permission;
use crate::post::cache::PostImageCache;
use crate::RequirePermissionContext;
use chrono::Days;
use chrono::Utc;
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::sync::atomic;
use tide::log::error;
use tide::prelude::*;
use tide::Request;
use tide::StatusCode;

use sms3rs_shared::post::handle::*;

/// Read and store a cache image with cache id returned.
pub async fn cache_image(mut req: Request<()>) -> tide::Result {
    let cxt = match RequirePermissionContext::from_header(&req) {
        Some(e) => e,
        None => {
            return Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": "Permission denied",
                })
                .into(),
            )
        }
    };
    match cxt.valid(vec![Permission::Post]).await {
        Ok(able) => {
            if able {
                let id;
                super::cache::INSTANCE
                    .push(
                        match PostImageCache::new(
                            &{
                                let bs = req.body_bytes().await?;
                                if bs.len() > 50_000_000 {
                                    return Ok::<tide::Response, tide::Error>(
                                        json!({
                                            "status": "error",
                                            "error": format!("Image too big"),
                                        })
                                        .into(),
                                    );
                                }
                                bs
                            },
                            cxt.account_id,
                        ) {
                            Ok(e) => {
                                id = e.1;
                                e.0
                            }
                            Err(err) => {
                                return Ok::<tide::Response, tide::Error>(
                                    json!({
                                        "status": "error",
                                        "error": format!("Image cache create failed: {}", err),
                                    })
                                    .into(),
                                )
                            }
                        },
                    )
                    .await;
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "success",
                        "hash": id,
                    })
                    .into(),
                )
            } else {
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Permission denied",
                    })
                    .into(),
                )
            }
        }
        Err(err) => Ok::<tide::Response, tide::Error>(
            json!({
                "status": "error",
                "error": err.to_string(),
            })
            .into(),
        ),
    }
}

/// Get image png bytes from target image cache hash.
pub async fn get_image(mut req: Request<()>) -> tide::Result {
    let cxt = match RequirePermissionContext::from_header(&req) {
        Some(e) => e,
        None => {
            return Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": "Permission denied",
                })
                .into(),
            )
        }
    };
    let descriptor: GetImageDescriptor = req.body_json().await?;
    match cxt.valid(vec![Permission::View]).await {
        Ok(able) => {
            if able {
                for img in super::cache::INSTANCE.caches.read().await.iter() {
                    if img.hash == descriptor.hash {
                        return Ok::<tide::Response, tide::Error>({
                            #[allow(unused_mut)]
                            let mut rep = tide::Response::new(StatusCode::Ok);

                            #[cfg(not(test))]
                            {
                                use async_std::io::BufReader;
                                use tide::Body;
                                rep.set_body(Body::from_reader(
                                    match async_std::fs::File::open(format!(
                                        "./data/images/{}.png",
                                        img.hash
                                    ))
                                    .await
                                    {
                                        Ok(e) => BufReader::new(e),
                                        Err(_) => {
                                            return Ok::<tide::Response, tide::Error>(
                                                tide::Response::new(StatusCode::NoContent),
                                            )
                                        }
                                    },
                                    None,
                                ));
                            }

                            rep
                        });
                    }
                }
                Ok::<tide::Response, tide::Error>(tide::Response::new(StatusCode::NoContent))
            } else {
                Ok::<tide::Response, tide::Error>(tide::Response::new(StatusCode::NoContent))
            }
        }
        Err(err) => Ok::<tide::Response, tide::Error>(
            json!({
                "status": "error",
                "error": err.to_string(),
            })
            .into(),
        ),
    }
}

pub async fn new_post(mut req: Request<()>) -> tide::Result {
    let cxt = match RequirePermissionContext::from_header(&req) {
        Some(e) => e,
        None => {
            return Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": "Permission denied",
                })
                .into(),
            )
        }
    };
    let descriptor: PostDescriptor = req.body_json().await?;
    match cxt.valid(vec![Permission::Post]).await {
        Ok(able) => {
            if able {
                let cache = super::cache::INSTANCE.caches.read().await;
                for img_id in descriptor.images.iter() {
                    if !cache.iter().any(|e| e.hash == *img_id) {
                        return Ok::<tide::Response, tide::Error>(
                            json!({
                                "status": "error",
                                "error": format!("Target image cache {} not fount", img_id),
                            })
                            .into(),
                        );
                    }
                }
                for img_id in descriptor.images.iter() {
                    cache
                        .iter()
                        .find(|e| e.hash == *img_id)
                        .unwrap()
                        .blocked
                        .store(true, atomic::Ordering::Relaxed)
                }
                let post = Post {
                    id: {
                        let mut hasher = DefaultHasher::new();
                        descriptor.title.hash(&mut hasher);
                        descriptor.description.hash(&mut hasher);
                        descriptor.images.hash(&mut hasher);
                        let id = hasher.finish();
                        if super::INSTANCE.contains_id(id).await {
                            return Ok::<tide::Response, tide::Error>(
                                json!({
                                    "status": "error",
                                    "error": "Post id repeated",
                                })
                                .into(),
                            );
                        }
                        id
                    },
                    images: descriptor.images,
                    status: {
                        let mut deque = VecDeque::new();
                        deque.push_back(super::PostAcceptationData {
                            operator: cxt.account_id,
                            status: super::PostAcceptationStatus::Pending,
                            time: Utc::now(),
                        });
                        deque
                    },
                    metadata: super::PostMetadata {
                        title: descriptor.title,
                        description: descriptor.description,
                        time_range: {
                            if descriptor
                                .time_range
                                .0
                                .checked_add_days(Days::new(7))
                                .map_or(false, |e| e < descriptor.time_range.1)
                            {
                                return Ok::<tide::Response, tide::Error>(
                                    json!({
                                        "status": "error",
                                        "error": "Post time out of range",
                                    })
                                    .into(),
                                );
                            }
                            descriptor.time_range
                        },
                    },
                    publisher: cxt.account_id,
                };
                if !super::save_post(&post).await {
                    error!("Error while saving post {}", post.id);
                }
                let id = post.id;
                super::INSTANCE.push(post).await;
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "success",
                        "post_id": id,
                    })
                    .into(),
                )
            } else {
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Permission denied",
                    })
                    .into(),
                )
            }
        }
        Err(err) => Ok::<tide::Response, tide::Error>(
            json!({
                "status": "error",
                "error": err.to_string(),
            })
            .into(),
        ),
    }
}

pub async fn get_posts(mut req: Request<()>) -> tide::Result {
    let cxt = match RequirePermissionContext::from_header(&req) {
        Some(e) => e,
        None => {
            return Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": "Permission denied",
                })
                .into(),
            )
        }
    };
    let descriptor: GetPostsDescriptor = req.body_json().await?;
    match cxt.valid(vec![Permission::View]).await {
        Ok(able) => {
            if able {
                let am = account::INSTANCE.inner().read().await;
                let ar = am
                    .get(
                        *account::INSTANCE
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
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "success",
                        "posts": posts,
                    })
                    .into(),
                )
            } else {
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Permission denied",
                    })
                    .into(),
                )
            }
        }
        Err(err) => Ok::<tide::Response, tide::Error>(
            json!({
                "status": "error",
                "error": err.to_string(),
            })
            .into(),
        ),
    }
}

/// If the target post matches this filter and the target account has enough permission to get the post.
fn matches_get_post_filter(filter: &GetPostsFilter, post: &Post, user: &Account) -> bool {
    let date = Utc::now().date_naive();
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
    }) && (post.publisher == user.id()
        || (post.metadata.time_range.0 <= date && user.has_permission(Permission::View))
        || user.has_permission(Permission::Check))
}

pub async fn edit_post(mut req: Request<()>) -> tide::Result {
    let cxt = match RequirePermissionContext::from_header(&req) {
        Some(e) => e,
        None => {
            return Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": "Permission denied",
                })
                .into(),
            )
        }
    };
    let descriptor: EditPostDescriptor = req.body_json().await?;
    match cxt.valid(vec![Permission::Post]).await {
        Ok(able) => {
            if able {
                for p in super::INSTANCE.posts.read().await.iter() {
                    let pr = p.read().await;
                    if pr.id == descriptor.post {
                        if pr.publisher != cxt.account_id
                            || pr
                                .status
                                .back()
                                .map(|e| matches!(e.status, PostAcceptationStatus::Submitted(_)))
                                .unwrap_or_default()
                        {
                            return Ok::<tide::Response, tide::Error>(
                                json!({
                                    "status": "error",
                                    "error": "Permission denied",
                                })
                                .into(),
                            );
                        }
                        let id = pr.id;
                        drop(pr);
                        for variant in descriptor.variants.iter() {
                            match apply_edit_post_variant(variant, id, cxt.account_id).await {
                                Some(err) => {
                                    return Ok::<tide::Response, tide::Error>(
                                        json!({
                                            "status": "error",
                                            "error": format!("Error occurred with post variant {id}: {err}"),
                                        })
                                        .into(),
                                    );
                                }
                                _ => (),
                            }
                        }
                        let post = p.read().await;
                        if !super::save_post(post.deref()).await {
                            error!("Error while saving post {}", post.id);
                        }
                        return Ok::<tide::Response, tide::Error>(
                            json!({
                                "status": "success",
                            })
                            .into(),
                        );
                    }
                }
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Target post not found",
                    })
                    .into(),
                )
            } else {
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Permission denied",
                    })
                    .into(),
                )
            }
        }
        Err(err) => Ok::<tide::Response, tide::Error>(
            json!({
                "status": "error",
                "error": err.to_string(),
            })
            .into(),
        ),
    }
}

/// Apply this edition, return an err if error occurs.
pub async fn apply_edit_post_variant(
    variant: &EditPostVariant,
    post_id: u64,
    user_id: u64,
) -> Option<String> {
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
        None => return Some("Post not found".to_string()),
    };
    match variant {
        EditPostVariant::Title(value) => post.metadata.title = value.clone(),
        EditPostVariant::Description(value) => post.metadata.description = value.clone(),
        EditPostVariant::Images(imgs) => {
            let cache = super::cache::INSTANCE.caches.read().await;
            for img_id in post.images.iter() {
                if let Some(e) = cache.iter().find(|e| e.hash == *img_id) {
                    e.blocked.store(false, atomic::Ordering::Relaxed)
                }
            }
            for img_id in imgs.iter() {
                if !cache.iter().any(|e| e.hash == *img_id) {
                    return Some(format!("Target image cache {} not fount", img_id));
                }
            }
            for img_id in imgs.iter() {
                let mut unlock = true;
                for e in super::INSTANCE.posts.read().await.iter() {
                    if let Some(er) = e.try_read() {
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
                        .store(true, atomic::Ordering::Relaxed)
                }
            }
        }
        EditPostVariant::TimeRange(start, end) => {
            if start
                .checked_add_days(Days::new(7))
                .map_or(false, |e| &e < end)
            {
                return Some("Post time out of range".to_string());
            }
            post.metadata.time_range = (*start, *end);
        }
        EditPostVariant::CancelSubmission => {
            if post
                .status
                .back()
                .map_or(true, |e| matches!(e.status, PostAcceptationStatus::Pending))
            {
                return Some("Target post was already pended".to_string());
            }
            post.status.push_back(super::PostAcceptationData {
                operator: user_id,
                status: PostAcceptationStatus::Pending,
                time: Utc::now(),
            })
        }
        EditPostVariant::Destroy => {
            if !super::remove_post(post.deref()).await {
                error!("Post {} save failed", post.id);
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
                            if let Some(er) = e.try_read() {
                                if er.images.contains(img_id) {
                                    unlock = false;
                                    break;
                                }
                            }
                        }

                        if unlock {
                            for im in super::cache::INSTANCE.caches.read().await.iter() {
                                if &im.hash == img_id {
                                    im.blocked.store(false, atomic::Ordering::Relaxed);
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
                None => return Some("Post not found".to_string()),
            }
        }
        EditPostVariant::RequestReview(msg) => {
            if post.status.back().map_or(true, |e| {
                matches!(e.status, PostAcceptationStatus::Submitted(_))
            }) {
                return Some("Target post was already submitted".to_string());
            }
            post.status.push_back(super::PostAcceptationData {
                operator: user_id,
                status: PostAcceptationStatus::Submitted(msg.clone()),
                time: Utc::now(),
            })
        }
    }
    None
}

pub async fn get_posts_info(mut req: Request<()>) -> tide::Result {
    let cxt = match RequirePermissionContext::from_header(&req) {
        Some(e) => e,
        None => {
            return Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": "Permission denied",
                })
                .into(),
            )
        }
    };
    let descriptor: GetPostsInfoDescriptor = req.body_json().await?;
    match cxt.valid(vec![Permission::View]).await {
        Ok(able) => {
            if able {
                let am = account::INSTANCE.inner().read().await;
                let ar = am
                    .get(
                        *account::INSTANCE
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
                let date = Utc::now().date_naive();
                for p in descriptor.posts.iter() {
                    let mut ps = false;
                    for e in posts.iter() {
                        let er = e.read().await;
                        if er.id == *p {
                            if er.publisher == cxt.account_id
                                || ar.has_permission(Permission::Check)
                            {
                                results.push(GetPostInfoResult::Full(er.clone()))
                            } else if er.metadata.time_range.0 <= date
                                && ar.has_permission(Permission::View)
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
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "success",
                        "results": results,
                    })
                    .into(),
                )
            } else {
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Permission denied",
                    })
                    .into(),
                )
            }
        }
        Err(err) => Ok::<tide::Response, tide::Error>(
            json!({
                "status": "error",
                "error": err.to_string(),
            })
            .into(),
        ),
    }
}

pub async fn approve_post(mut req: Request<()>) -> tide::Result {
    let cxt = match RequirePermissionContext::from_header(&req) {
        Some(e) => e,
        None => {
            return Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": "Permission denied",
                })
                .into(),
            )
        }
    };
    let descriptor: ApprovePostDescriptor = req.body_json().await?;
    match cxt.valid(vec![Permission::Approve]).await {
        Ok(able) => {
            if able {
                let posts = super::INSTANCE.posts.read().await;
                for p in posts.iter() {
                    let pr = p.read().await;
                    if pr.id == descriptor.post {
                        drop(pr);
                        let mut pw = p.write().await;
                        match descriptor.variant {
                            ApprovePostVariant::Accept(msg) => {
                                if pw.status.back().map_or(false, |e| {
                                    matches!(e.status, PostAcceptationStatus::Accepted(_))
                                }) {
                                    return Ok::<tide::Response, tide::Error>(
                                        json!({
                                            "status": "error",
                                            "error": "Target post has already been accepted",
                                        })
                                        .into(),
                                    );
                                }
                                pw.status.push_back(super::PostAcceptationData {
                                    operator: cxt.account_id,
                                    status: PostAcceptationStatus::Accepted(
                                        msg.unwrap_or_default(),
                                    ),
                                    time: Utc::now(),
                                });
                            }
                            ApprovePostVariant::Reject(msg) => {
                                if pw.status.back().map_or(false, |e| {
                                    matches!(e.status, PostAcceptationStatus::Rejected(_))
                                }) {
                                    return Ok::<tide::Response, tide::Error>(
                                        json!({
                                            "status": "error",
                                            "error": "Target post has already been rejected",
                                        })
                                        .into(),
                                    );
                                }
                                pw.status.push_back(super::PostAcceptationData {
                                    operator: cxt.account_id,
                                    status: PostAcceptationStatus::Rejected({
                                        if msg.is_empty() {
                                            return Ok::<tide::Response, tide::Error>(
                                                json!({
                                                    "status": "error",
                                                    "error": "Message couldn't be empty",
                                                })
                                                .into(),
                                            );
                                        }
                                        msg
                                    }),
                                    time: Utc::now(),
                                });
                            }
                        };

                        if !super::save_post(pw.deref()).await {
                            error!("Error while saving post {}", pw.id);
                        }

                        return Ok::<tide::Response, tide::Error>(
                            json!({
                                "status": "success",
                            })
                            .into(),
                        );
                    }
                }
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Target post not found",
                    })
                    .into(),
                )
            } else {
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Permission denied",
                    })
                    .into(),
                )
            }
        }
        Err(err) => Ok::<tide::Response, tide::Error>(
            json!({
                "status": "error",
                "error": err.to_string(),
            })
            .into(),
        ),
    }
}
