use super::Post;
use super::PostAcceptationStatus;
use crate::account;
use crate::account::Account;
use crate::account::Permission;
use crate::post::cache::PostImageCache;
use crate::RequirePermissionContext;
use crate::ResError;
use axum::body::Bytes;
use axum::http::StatusCode;
use axum::Json;
use chrono::Days;
use chrono::Utc;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::sync::atomic;

use sms3rs_shared::post::handle::*;

/// Read and store a cache image with cache id returned.
pub async fn cache_image(
    (ctx, bytes): (RequirePermissionContext, Bytes),
) -> axum::response::Result<Json<serde_json::Value>> {
    ctx.valid(&[Permission::Post])
        .map_err(|err| ResError(err))?;

    let cache = PostImageCache::new(&bytes, ctx.account_id).map_err(|err| ResError(err))?;
    let id = cache.hash;

    super::cache::INSTANCE.push(cache);

    Ok(Json(json!({ "hash": id })))
}

/// Get image png bytes from target image cache hash.
pub async fn get_image(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<GetImageDescriptor>,
) -> axum::response::Result<Vec<u8>> {
    ctx.valid(&[Permission::View]).map_err(ResError)?;

    if let Some(_img) = super::cache::INSTANCE
        .caches
        .read()
        .iter()
        .find(|e| e.hash == descriptor.hash)
    {
        #[cfg(not(test))]
        return std::fs::File::open(format!("./data/images/{}.png", _img.hash))
            .map(|mut file| {
                let mut vec = Vec::new();
                let _ = std::io::Read::read_to_end(&mut file, &mut vec);

                vec
            })
            .map_err(|err| ResError(err).into());

        #[cfg(test)]
        unreachable!("test not covered");
    }

    Err(ResError(super::cache::Error::NotFound).into())
}

pub async fn new_post(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<PostDescriptor>,
) -> axum::response::Result<Json<serde_json::Value>> {
    ctx.valid(&[Permission::Post]).map_err(ResError)?;
    let cache = super::cache::INSTANCE.caches.read();

    if descriptor
        .images
        .iter()
        .any(|img_id| !cache.iter().any(|e| e.hash == *img_id))
    {
        return Err(ResError(super::cache::Error::NotFound).into());
    }

    descriptor.images.iter().for_each(|img_id| {
        cache
            .iter()
            .find(|e| e.hash == *img_id)
            .unwrap()
            .blocked
            .store(true, atomic::Ordering::Release)
    });

    let post = Post {
        id: {
            let mut hasher = DefaultHasher::new();

            descriptor.title.hash(&mut hasher);
            descriptor.description.hash(&mut hasher);
            descriptor.images.hash(&mut hasher);

            let id = hasher.finish();

            if super::INSTANCE.contains_id(id) {
                return Err(ResError(super::Error::Conflict).into());
            }

            id
        },

        status: {
            let mut vec = Vec::new();
            vec.push(super::PostAcceptationData {
                operator: ctx.account_id,
                status: super::PostAcceptationStatus::Pending,
                time: Utc::now(),
            });
            vec
        },

        metadata: super::PostMetadata {
            title: descriptor.title,
            description: descriptor.description,

            time_range: {
                if descriptor.time_range.0 + Days::new(7) < descriptor.time_range.1 {
                    return Err(ResError(super::Error::DateOutOfRange).into());
                }

                descriptor.time_range
            },
        },

        images: descriptor.images,
        publisher: ctx.account_id,
    };

    super::save_post(&post);

    let id = post.id;
    super::INSTANCE.push(post);

    Ok(Json(json!({ "post_id": id })))
}

pub async fn get_posts(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<GetPostsDescriptor>,
) -> axum::response::Result<Json<serde_json::Value>> {
    ctx.valid(&[Permission::View]).map_err(ResError)?;

    let am = account::INSTANCE.inner().read();
    let ar = am
        .get(*account::INSTANCE.index().get(&ctx.account_id).unwrap())
        .unwrap()
        .read();

    let mut posts = Vec::new();

    super::INSTANCE.posts.read().iter().for_each(|p| {
        let pr = p.read();

        if descriptor
            .filters
            .iter()
            .all(|f| matches_get_post_filter(f, pr.deref(), ar.deref()))
        {
            posts.push(pr.id);
        }
    });

    Ok(Json(json!({ "posts": posts })))
}

/// If the target post matches this filter and the target account
/// has enough permission to get the post.
fn matches_get_post_filter(filter: &GetPostsFilter, post: &Post, user: &Account) -> bool {
    let date = Utc::now().date_naive();

    (match filter {
        GetPostsFilter::Acceptation(status) => post
            .status
            .last()
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

pub async fn edit_post(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<EditPostDescriptor>,
) -> axum::response::Result<()> {
    ctx.valid(&[Permission::Post]).map_err(ResError)?;

    if let Some(p) = super::INSTANCE
        .posts
        .read()
        .iter()
        .find(|p| p.read().id == descriptor.post)
    {
        let pr = p.read();

        if pr.publisher != ctx.account_id
            || pr
                .status
                .last()
                .map(|e| matches!(e.status, PostAcceptationStatus::Submitted(_)))
                .unwrap_or_default()
        {
            return Err(ResError(crate::account::Error::PermissionDenied).into());
        }

        drop(pr);

        for variant in descriptor.variants.iter() {
            apply_edit_post_variant(variant, descriptor.post, ctx.account_id).map_err(ResError)?;
        }

        let post = p.read();
        super::save_post(post.deref());

        Ok(())
    } else {
        return Err(ResError(super::Error::NotFound).into());
    }
}

/// Apply this edition, return an err if error occurs.
fn apply_edit_post_variant(
    variant: &EditPostVariant,
    post_id: u64,
    user_id: u64,
) -> Result<(), super::Error> {
    let posts = super::INSTANCE.posts.read();

    let mut post;

    post = posts
        .iter()
        .find(|p| p.read().id == post_id)
        .ok_or(super::Error::NotFound)?
        .write();

    match variant {
        EditPostVariant::Title(value) => post.metadata.title = value.clone(),
        EditPostVariant::Description(value) => post.metadata.description = value.clone(),

        EditPostVariant::Images(imgs) => {
            let cache = super::cache::INSTANCE.caches.read();
            for img_id in post.images.iter() {
                if let Some(e) = cache.iter().find(|e| e.hash == *img_id) {
                    e.blocked.store(false, atomic::Ordering::Release)
                }
            }
            for img_id in imgs.iter() {
                if !cache.iter().any(|e| e.hash == *img_id) {
                    return Err(super::Error::Cache(super::cache::Error::NotFound));
                }
            }
            for img_id in imgs.iter() {
                let mut unlock = true;
                for e in super::INSTANCE.posts.read().iter() {
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
                        .store(true, atomic::Ordering::Release)
                }
            }
        }

        EditPostVariant::TimeRange(start, end) => {
            if start
                .checked_add_days(Days::new(7))
                .map_or(false, |e| &e < end)
            {
                return Err(super::Error::DateOutOfRange);
            }
            post.metadata.time_range = (*start, *end);
        }

        EditPostVariant::CancelSubmission => {
            if post
                .status
                .last()
                .map_or(true, |e| matches!(e.status, PostAcceptationStatus::Pending))
            {
                return Err(super::Error::AlreadyInStatus(
                    PostAcceptationStatus::Pending,
                ));
            }

            post.status.push(super::PostAcceptationData {
                operator: user_id,
                status: PostAcceptationStatus::Pending,
                time: Utc::now(),
            })
        }

        EditPostVariant::Destroy => {
            drop(post);
            drop(posts);

            let mut posts = super::INSTANCE.posts.write();

            if let Some(post) = posts.iter().enumerate().find(|e| e.1.read().id == post_id) {
                let pr = post.1.read();

                for img_id in pr.images.iter() {
                    let mut unlock = true;
                    for e in super::INSTANCE.posts.read().iter() {
                        if e.read().images.contains(img_id) {
                            unlock = false;
                            break;
                        }
                    }

                    if unlock {
                        for im in super::cache::INSTANCE.caches.read().iter() {
                            if im.hash == *img_id {
                                im.blocked.store(false, atomic::Ordering::Release);
                                break;
                            }
                        }
                    }
                }

                super::remove_post(pr.deref());

                let i = post.0;
                drop(pr);
                posts.remove(i);
            } else {
                return Err(super::Error::NotFound);
            }
        }

        EditPostVariant::RequestReview(msg) => {
            if let Some(sms3rs_shared::post::PostAcceptationData { status, .. }) =
                post.status.last()
            {
                if let PostAcceptationStatus::Submitted(msg1) = status {
                    return Err(super::Error::AlreadyInStatus(
                        PostAcceptationStatus::Submitted(msg1.to_string()),
                    ));
                }
            }

            post.status.push(super::PostAcceptationData {
                operator: user_id,
                status: PostAcceptationStatus::Submitted(msg.clone()),
                time: Utc::now(),
            })
        }
    }

    Ok(())
}

pub async fn get_posts_info(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<GetPostsInfoDescriptor>,
) -> (StatusCode, Json<serde_json::Value>) {
    if ctx.try_valid(&[Permission::View]).unwrap() {
        let am = account::INSTANCE.inner().read();
        let ar = am
            .get(*account::INSTANCE.index().get(&ctx.account_id).unwrap())
            .unwrap()
            .read();

        let mut results = Vec::new();
        let posts = super::INSTANCE.posts.read();
        let date = Utc::now().date_naive();

        for p in descriptor.posts.iter() {
            let mut ps = false;

            for e in posts.iter() {
                let er = e.read();

                if er.id == *p {
                    if er.publisher == ctx.account_id || ar.has_permission(Permission::Check) {
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

        (StatusCode::OK, Json(json!({ "results": results })))
    } else {
        (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "permission denied" })),
        )
    }
}

pub async fn approve_post(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<ApprovePostDescriptor>,
) -> (StatusCode, Json<serde_json::Value>) {
    if ctx.try_valid(&[Permission::Approve]).unwrap() {
        let posts = super::INSTANCE.posts.read();
        if let Some(p) = posts.iter().find(|e| e.read().id == descriptor.post) {
            let mut pw = p.write();
            match descriptor.variant {
                ApprovePostVariant::Accept(msg) => {
                    if pw.status.last().map_or(false, |e| {
                        matches!(e.status, PostAcceptationStatus::Accepted(_))
                    }) {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(json!({ "error": "target post has already been accepted" })),
                        );
                    }

                    pw.status.push(super::PostAcceptationData {
                        operator: ctx.account_id,
                        status: PostAcceptationStatus::Accepted(msg.unwrap_or_default()),
                        time: Utc::now(),
                    });
                }

                ApprovePostVariant::Reject(msg) => {
                    if pw.status.last().map_or(false, |e| {
                        matches!(e.status, PostAcceptationStatus::Rejected(_))
                    }) {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(json!({ "error": "target post has already been rejected" })),
                        );
                    }

                    pw.status.push(super::PostAcceptationData {
                        operator: ctx.account_id,

                        status: PostAcceptationStatus::Rejected({
                            if msg.is_empty() {
                                return (
                                    StatusCode::FORBIDDEN,
                                    Json(json!({ "error": "message body could not be empty" })),
                                );
                            }

                            msg
                        }),

                        time: Utc::now(),
                    });
                }
            };

            super::save_post(pw.deref());
            return (StatusCode::OK, Json(json!({})));
        }

        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "target post not found" })),
        )
    } else {
        (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "permission denied" })),
        )
    }
}
