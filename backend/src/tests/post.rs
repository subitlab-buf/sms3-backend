use super::*;

use axum::http;
use hyper::{Request, StatusCode};
use serial_test::serial;
use sha256::digest;
use tower::ServiceExt;

const TEST_POST_IMG: &[u8; 108416] = include_bytes!("../../../test-resources/test_post.png");

#[serial]
#[tokio::test]
async fn cache_image() {
    reset_all();

    let app = crate::router();

    let account_id = 123456;
    let password = "password123456";

    let token;

    crate::account::INSTANCE.push(crate::account::Account::Verified {
        id: account_id,
        attributes: crate::account::UserAttributes {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
            name: "Jiening Yu".to_string(),
            school_id: 2522320,
            house: Some(sms3_shared::account::House::ZhiZhi),
            phone: 16601550826,
            organization: None,
            permissions: vec![crate::account::Permission::Post],
            registration_time: chrono::Utc::now(),
            password_sha: digest(password.to_string()),
            token_expiration_time: 0,
        },
        tokens: {
            let mut t = crate::account::verify::Tokens::new();
            token = t.new_token(account_id, 0);
            t
        },
        verify: crate::account::UserVerifyVariant::None,
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/post/upload-image")
                .method("POST")
                .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .header("Token", &token)
                .header("AccountId", account_id)
                .body(TEST_POST_IMG.as_slice().into())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_json: serde_json::Value =
        serde_json::from_slice(&hyper::body::to_bytes(response.into_body()).await.unwrap())
            .unwrap();

    assert_eq!(
        response_json
            .as_object()
            .unwrap()
            .get("hash")
            .unwrap()
            .as_u64()
            .unwrap(),
        crate::post::cache::INSTANCE.caches.read()[0].hash
    );
}

#[serial]
#[tokio::test]
async fn new() {
    reset_all();

    let app = crate::router();

    let account_id = 123456;
    let password = "password123456";

    let token;

    crate::account::INSTANCE.push(crate::account::Account::Verified {
        id: account_id,
        attributes: crate::account::UserAttributes {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
            name: "Jiening Yu".to_string(),
            school_id: 2522320,
            house: Some(sms3_shared::account::House::ZhiZhi),
            phone: 16601550826,
            organization: None,
            permissions: vec![sms3_shared::account::Permission::Post],
            registration_time: chrono::Utc::now(),
            password_sha: digest(password.to_string()),
            token_expiration_time: 0,
        },
        tokens: {
            let mut t = crate::account::verify::Tokens::new();
            token = t.new_token(account_id, 0);
            t
        },
        verify: crate::account::UserVerifyVariant::None,
    });

    let cache_id = 1;

    crate::post::cache::INSTANCE
        .caches
        .write()
        .push(crate::post::cache::PostImageCache {
            hash: cache_id,
            uploader: account_id,
            blocked: std::sync::atomic::AtomicBool::new(false),
            img: parking_lot::RwLock::new(None),
        });

    use sms3_shared::post::handle::PostDescriptor;

    let descriptor = PostDescriptor {
        title: "Test post".to_string(),
        description: "Just for testing".to_string(),
        time_range: (
            chrono::Utc::now().date_naive(),
            chrono::Utc::now().date_naive() + chrono::Days::new(1),
        ),
        images: vec![cache_id],
    };

    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/post/create")
                    .method("POST")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .header("Token", &token)
                    .header("AccountId", account_id)
                    .body(serde_json::to_vec(&descriptor).unwrap().into())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    assert!(crate::post::cache::INSTANCE.caches.read()[0]
        .blocked
        .load(std::sync::atomic::Ordering::Acquire));

    assert!(!crate::post::INSTANCE.posts.read().is_empty());
}
