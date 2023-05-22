use super::Post;
use super::PostAcceptationStatus;
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

#[derive(Serialize, Deserialize)]
struct PostDescriptor {
    title: String,
    description: String,
    time_range: (NaiveDate, NaiveDate),
    images: Vec<u64>,
}

pub async fn view_self_post(req: Request<()>) -> tide::Result {
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
    match cxt.valid(vec![]).await {
        Ok(able) => {
            if able {
                let mut posts = Vec::new();
                for p in super::INSTANCE.posts.read().await.iter() {
                    let pr = p.read().await;
                    if pr.publisher == cxt.user_id {
                        posts.push(pr.clone());
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

/// Request a review to admins
/// by adding `Submitted` to the target status deque.
pub async fn request_review(mut req: Request<()>) -> tide::Result {
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
    let descriptor: RequestReviewDescriptor = req.body_json().await?;
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
                        drop(pr);
                        let mut pw = p.write().await;
                        pw.status.push_back(super::PostAcceptationData {
                            operator: cxt.user_id,
                            status: PostAcceptationStatus::Submitted(
                                descriptor.message.unwrap_or_default(),
                            ),
                            time: Utc::now(),
                        });
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

#[derive(Serialize, Deserialize)]
struct RequestReviewDescriptor {
    post: u64,
    /// The message for admins.
    message: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct EditPostDescriptor {
    post: u64,
    variants: Vec<EditPostVariant>,
}

#[derive(Serialize, Deserialize)]
enum EditPostVariant {
    Title(String),
    Description(String),
    Images(Vec<u64>),
    TimeRange(NaiveDate, NaiveDate),
    /// Change status of the post to `Pending`
    /// if the target status is `Submitted`.
    CancelSubmittion,
    /// Remove the post and unblock all the images it use.
    Destroy,
}
