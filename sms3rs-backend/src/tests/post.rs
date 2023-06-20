use super::*;

use serial_test::serial;

const TEST_POST_IMG: &[u8; 108416] = include_bytes!("../../../test-resources/test_post.png");

#[serial]
#[tokio::test]
async fn cache_image() {
    reset_all().await;

    let app = actix_web::test::init_service(
        actix_web::App::new().service(crate::post::handle::cache_image),
    )
    .await;

    let account_id = 123456;
    let password = "password123456";

    let token;

    crate::account::INSTANCE
        .push(crate::account::Account::Verified {
            id: account_id,
            attributes: crate::account::UserAttributes {
                email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                name: "Jiening Yu".to_string(),
                school_id: 2522320,
                house: Some(sms3rs_shared::account::House::ZhiZhi),
                phone: 16601550826,
                organization: None,
                permissions: vec![crate::account::Permission::Post],
                registration_time: chrono::Utc::now(),
                password_sha: sha256::digest(password.to_string()),
                token_expiration_time: 0,
            },
            tokens: {
                let mut t = crate::account::verify::Tokens::new();
                token = t.new_token(account_id, 0);
                t
            },
            verify: crate::account::UserVerifyVariant::None,
        })
        .await;

    let response = actix_web::test::call_service(
        &app,
        actix_web::test::TestRequest::post()
            .uri("/api/post/cache")
            .insert_header(crate::RequirePermissionContext {
                account_id,
                token: token.to_string(),
            })
            .set_payload(actix_web::web::Bytes::from_static(TEST_POST_IMG))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), actix_web::http::StatusCode::OK);

    let response_json: serde_json::Value = actix_web::test::read_body_json(response).await;

    assert_eq!(
        response_json
            .as_object()
            .unwrap()
            .get("hash")
            .unwrap()
            .as_u64()
            .unwrap(),
        crate::post::cache::INSTANCE.caches.read().await[0].hash
    );
}

#[serial]
#[tokio::test]
async fn new() {
    reset_all().await;

    let app = actix_web::test::init_service(
        actix_web::App::new().service(crate::post::handle::create_post),
    )
    .await;

    let account_id = 123456;
    let password = "password123456";

    let token;

    crate::account::INSTANCE
        .push(crate::account::Account::Verified {
            id: account_id,
            attributes: crate::account::UserAttributes {
                email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                name: "Jiening Yu".to_string(),
                school_id: 2522320,
                house: Some(sms3rs_shared::account::House::ZhiZhi),
                phone: 16601550826,
                organization: None,
                permissions: vec![sms3rs_shared::account::Permission::Post],
                registration_time: chrono::Utc::now(),
                password_sha: sha256::digest(password.to_string()),
                token_expiration_time: 0,
            },
            tokens: {
                let mut t = crate::account::verify::Tokens::new();
                token = t.new_token(account_id, 0);
                t
            },
            verify: crate::account::UserVerifyVariant::None,
        })
        .await;

    let cache_id = 1;

    crate::post::cache::INSTANCE
        .caches
        .write()
        .await
        .push(crate::post::cache::PostImageCache {
            hash: cache_id,
            uploader: account_id,
            blocked: std::sync::atomic::AtomicBool::new(false),
            img: tokio::sync::RwLock::new(None),
        });

    use sms3rs_shared::post::handle::CreatePostDescriptor;

    assert_eq!(
        actix_web::test::call_service(
            &app,
            actix_web::test::TestRequest::post()
                .uri("/api/post/new")
                .insert_header(crate::RequirePermissionContext {
                    account_id,
                    token: token.to_string(),
                })
                .set_json(CreatePostDescriptor {
                    title: "Test post".to_string(),
                    description: "Just for testing".to_string(),
                    time_range: (
                        chrono::Utc::now().date_naive(),
                        chrono::Utc::now().date_naive() + chrono::Days::new(1),
                    ),
                    images: vec![cache_id],
                })
                .to_request(),
        )
        .await
        .status(),
        actix_web::http::StatusCode::OK
    );

    assert!(crate::post::cache::INSTANCE.caches.read().await[0]
        .blocked
        .load(std::sync::atomic::Ordering::Relaxed));

    assert!(!crate::post::INSTANCE.posts.read().await.is_empty());
}
