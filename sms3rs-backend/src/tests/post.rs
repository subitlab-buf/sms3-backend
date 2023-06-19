use super::*;

use serial_test::serial;
use sha256::digest;
use tide_testing::TideTestingExt;

const TEST_POST_IMG: &[u8; 108416] = include_bytes!("../../../test-resources/test_post.png");

#[serial]
#[async_std::test]
async fn cache_image() {
    reset_all().await;

    let mut app = tide::new();
    app.at("/api/post/cache")
        .post(crate::post::handle::cache_image);

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
                registration_ip: Some("127.0.0.1".to_string()),
                password_sha: digest(password.to_string()),
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

    let response_json: serde_json::Value = app
        .post("/api/post/cache")
        .header("Token", token.to_string())
        .header("AccountId", account_id.to_string())
        .body_bytes(TEST_POST_IMG)
        .recv_json()
        .await
        .unwrap();

    assert_eq!(
        response_json
            .as_object()
            .unwrap()
            .get("status")
            .unwrap()
            .as_str()
            .unwrap(),
        "success"
    );

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
#[async_std::test]
async fn new() {
    reset_all().await;

    let mut app = tide::new();
    app.at("/api/post/new")
        .post(crate::post::handle::create_post);

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
                registration_ip: Some("127.0.0.1".to_string()),
                password_sha: digest(password.to_string()),
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
            img: async_std::sync::RwLock::new(None),
        });

    use sms3rs_shared::post::handle::CreatePostDescriptor;

    let descriptor = CreatePostDescriptor {
        title: "Test post".to_string(),
        description: "Just for testing".to_string(),
        time_range: (
            chrono::Utc::now().date_naive(),
            chrono::Utc::now().date_naive() + chrono::Days::new(1),
        ),
        images: vec![cache_id],
    };

    let response_json: serde_json::Value = app
        .post("/api/post/new")
        .header("Token", token.to_string())
        .header("AccountId", account_id.to_string())
        .body_json(&descriptor)
        .unwrap()
        .recv_json()
        .await
        .unwrap();

    assert_eq!(
        response_json
            .as_object()
            .unwrap()
            .get("status")
            .unwrap()
            .as_str()
            .unwrap(),
        "success"
    );

    assert!(crate::post::cache::INSTANCE.caches.read().await[0]
        .blocked
        .load(std::sync::atomic::Ordering::Relaxed));

    assert!(!crate::post::INSTANCE.posts.read().await.is_empty());
}
