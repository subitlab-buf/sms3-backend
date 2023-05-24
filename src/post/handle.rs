use super::Post;
use super::PostAcceptationStatus;
use crate::account;
use crate::account::Account;
use crate::account::Permission;
use crate::post::cache::PostImageCache;
use crate::RequirePermissionContext;
use chrono::Days;
use chrono::NaiveDate;
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
                        match PostImageCache::new(&req.body_bytes().await?, cxt.user_id) {
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
                        "cache_id": id,
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
                            operator: cxt.user_id,
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
                                .map_or(false, |e| e > descriptor.time_range.1)
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
                    publisher: cxt.user_id,
                };
                if !post.save() {
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

#[derive(Deserialize)]
struct PostDescriptor {
    title: String,
    description: String,
    time_range: (NaiveDate, NaiveDate),
    images: Vec<u64>,
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
    match cxt.valid(vec![]).await {
        Ok(able) => {
            if able {
                let am = account::INSTANCE.inner().read().await;
                let ar = am
                    .get(
                        *account::INSTANCE
                            .index()
                            .read()
                            .await
                            .get(&cxt.user_id)
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
                        .all(|f| f.matches(pr.deref(), ar.deref()))
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

#[derive(Deserialize)]
struct GetPostsDescriptor {
    filters: Vec<GetPostsFilter>,
}

#[derive(Deserialize)]
enum GetPostsFilter {
    /// Posts that match target status.
    Acception(PostAcceptationStatus),
    /// Posts published by target account.
    Account(u64),
    After(NaiveDate),
    Before(NaiveDate),
    /// Posts which thier title and description contains target keywords.
    Keyword(String),
}

impl GetPostsFilter {
    /// If the target post matches this filter and the target account has enough permission to get the post.
    fn matches(&self, post: &Post, user: &Account) -> bool {
        let date = Utc::now().date_naive();
        (match self {
            GetPostsFilter::Acception(status) => {
                post.status.back().map_or(false, |s| &s.status == status)
            }
            GetPostsFilter::Account(account) => &post.publisher == account,
            GetPostsFilter::Before(d) => &post.metadata.time_range.0 <= d,
            GetPostsFilter::After(d) => &post.metadata.time_range.0 >= d,
            GetPostsFilter::Keyword(keywords) => {
                let ks = keywords.split_whitespace();
                ks.into_iter().all(|k| {
                    post.metadata.title.contains(k) && post.metadata.description.contains(k)
                })
            }
        }) && (post.publisher == user.id()
            || (post.metadata.time_range.0 <= date
                && post.metadata.time_range.1 >= date
                && user.has_permission(Permission::View))
            || user.has_permission(Permission::Check))
    }
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
                        if pr.publisher != cxt.user_id
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
                            match variant.apply(id, cxt.user_id).await {
                                Some(err) => {
                                    return Ok::<tide::Response, tide::Error>(
                                        json!({
                                            "status": "error",
                                            "error": format!("Error occured with post variant {id}: {err}"),
                                        })
                                        .into(),
                                    );
                                }
                                _ => (),
                            }
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

#[derive(Deserialize)]
struct EditPostDescriptor {
    post: u64,
    variants: Vec<EditPostVariant>,
}

#[derive(Deserialize)]
enum EditPostVariant {
    Title(String),
    Description(String),
    Images(Vec<u64>),
    TimeRange(NaiveDate, NaiveDate),
    /// Change status of the post to `Pending`
    /// if the target status is `Submitted`.
    CancelSubmittion,
    RequestReview(
        /// Message to admins.
        String,
    ),
    /// Remove the post and unblock all the images it use.
    Destroy,
}

impl EditPostVariant {
    /// Apply this edition, return an err if error occurs.
    pub async fn apply(&self, post_id: u64, user_id: u64) -> Option<String> {
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
        match self {
            EditPostVariant::Title(value) => post.metadata.title = value.clone(),
            EditPostVariant::Description(value) => post.metadata.description = value.clone(),
            EditPostVariant::Images(imgs) => {
                let cache = super::cache::INSTANCE.caches.read().await;
                for img_id in post.images.iter() {
                    cache
                        .iter()
                        .find(|e| e.hash == *img_id)
                        .map(|e| e.blocked.store(false, atomic::Ordering::Relaxed));
                }
                for img_id in imgs.iter() {
                    if !cache.iter().any(|e| e.hash == *img_id) {
                        return Some(format!("Target image cache {} not fount", img_id));
                    }
                }
                for img_id in imgs.iter() {
                    cache
                        .iter()
                        .find(|e| e.hash == *img_id)
                        .unwrap()
                        .blocked
                        .store(true, atomic::Ordering::Relaxed)
                }
            }
            EditPostVariant::TimeRange(start, end) => {
                if start
                    .checked_add_days(Days::new(7))
                    .map_or(false, |e| &e > end)
                {
                    return Some("Post time out of range".to_string());
                }
                post.metadata.time_range = (*start, *end);
            }
            EditPostVariant::CancelSubmittion => {
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
                if !post.remove() {
                    error!("Post {} save failed", post.id);
                }
                drop(post);
                drop(posts);
                let mut posts = super::INSTANCE.posts.write().await;
                let mut i = None;
                for post in posts.iter().enumerate() {
                    if post.1.read().await.id == post_id {
                        i = Some(post.0);
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
    match cxt.valid(vec![Permission::Post]).await {
        Ok(able) => {
            if able {
                let am = account::INSTANCE.inner().read().await;
                let ar = am
                    .get(
                        *account::INSTANCE
                            .index()
                            .read()
                            .await
                            .get(&cxt.user_id)
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
                            if er.publisher == cxt.user_id || ar.has_permission(Permission::Check) {
                                results.push(GetPostInfoResult::Full(er.clone()))
                            } else if er.metadata.time_range.0 <= date
                                && er.metadata.time_range.1 >= date
                                && ar.has_permission(Permission::View)
                            {
                                results.push(GetPostInfoResult::Forigen {
                                    id: er.id,
                                    images: er.images.clone(),
                                    title: er.metadata.title.clone(),
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

#[derive(Deserialize)]
struct GetPostsInfoDescriptor {
    posts: Vec<u64>,
}

#[derive(Serialize)]
enum GetPostInfoResult {
    Full(Post),
    Forigen {
        id: u64,
        images: Vec<u64>,
        title: String,
    },
    NotFound(
        /// Target post id
        u64,
    ),
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
                        return match descriptor.variant {
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
                                    operator: cxt.user_id,
                                    status: PostAcceptationStatus::Accepted(
                                        msg.unwrap_or_default(),
                                    ),
                                    time: Utc::now(),
                                });
                                Ok::<tide::Response, tide::Error>(
                                    json!({
                                        "status": "success",
                                    })
                                    .into(),
                                )
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
                                    operator: cxt.user_id,
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
                                Ok::<tide::Response, tide::Error>(
                                    json!({
                                        "status": "success",
                                    })
                                    .into(),
                                )
                            }
                        };
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

#[derive(Deserialize)]
struct ApprovePostDescriptor {
    post: u64,
    variant: ApprovePostVariant,
}

#[derive(Deserialize)]
enum ApprovePostVariant {
    Accept(
        /// Message
        Option<String>,
    ),
    Reject(
        /// Message, should not be empty.
        String,
    ),
}
