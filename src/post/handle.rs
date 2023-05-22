use super::Post;
use crate::account::Permission;
use crate::post::cache::PostImageCache;
use crate::RequirePermissionContext;
use chrono::NaiveDate;
use chrono::Utc;
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::Hash;
use std::hash::Hasher;
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

pub async fn post(mut req: Request<()>) -> tide::Result {
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
    let describer: PostDescriber = req.body_json().await?;
    match cxt.valid(vec![Permission::Post]).await {
        Ok(able) => {
            if able {
                let cache = super::cache::INSTANCE.caches.read().await;
                for img_id in describer.images.iter() {
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
                let post = Post {
                    id: {
                        let mut hasher = DefaultHasher::new();
                        describer.title.hash(&mut hasher);
                        describer.description.hash(&mut hasher);
                        describer.images.hash(&mut hasher);
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
                    images: describer.images,
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
                        title: describer.title,
                        description: describer.description,
                        time_range: describer.time_range,
                    },
                    requester: cxt.user_id,
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
struct PostDescriber {
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
    match cxt.valid(vec![Permission::Post]).await {
        Ok(able) => {
            if able {
                let mut posts = Vec::new();
                for p in super::INSTANCE.posts.read().await.iter() {
                    let pr = p.read().await;
                    if pr.requester == cxt.user_id {
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

#[derive(Serialize, Deserialize)]
struct RequestReviewDescriber {
    /// The post id.
    post: u64,
}
