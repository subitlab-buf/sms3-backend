use crate::account::Permission;
use crate::post::cache::PostImageCache;
use crate::post::Post;
use crate::RequirePermissionContext;
use std::todo;
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
