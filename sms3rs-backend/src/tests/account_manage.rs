use super::*;

use serial_test::serial;
use sha256::digest;
use tide_testing::TideTestingExt;

#[serial]
#[async_std::test]
async fn make() {
    reset_all().await;

    let mut app = tide::new();
    app.at("/api/account/manage/create")
        .post(crate::account::handle::manage::make_account);

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
                permissions: vec![sms3rs_shared::account::Permission::ManageAccounts],
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

    use sms3rs_shared::account::handle::manage::MakeAccountDescriptor;

    let descriptor = MakeAccountDescriptor {
        email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
        name: "Yuguo Ma".to_string(),
        school_id: 114514,
        phone: 1919810,
        house: None,
        organization: Some("PKU".to_string()),
        password: "password".to_string(),
        permissions: vec![
            sms3rs_shared::account::Permission::ManageAccounts,
            sms3rs_shared::account::Permission::OP,
        ],
    };

    let response_json: serde_json::Value = app
        .post("/api/account/manage/create")
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

    let instance = crate::account::INSTANCE.inner().read().await;
    let a = instance.get(
        *crate::account::INSTANCE
            .index()
            .read()
            .await
            .get(
                &response_json
                    .as_object()
                    .unwrap()
                    .get("account_id")
                    .unwrap()
                    .as_u64()
                    .unwrap(),
            )
            .unwrap(),
    );

    assert!(a.is_some());

    assert!(a
        .unwrap()
        .read()
        .await
        .has_permission(sms3rs_shared::account::Permission::ManageAccounts));
    // Test for permission overflowing
    assert!(!a
        .unwrap()
        .read()
        .await
        .has_permission(sms3rs_shared::account::Permission::OP));
}
