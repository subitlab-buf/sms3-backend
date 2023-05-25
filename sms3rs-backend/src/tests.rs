use serial_test::serial;
use sha256::digest;
use tide_testing::TideTestingExt;

/// Reset all static instances.
async fn reset_all() {
    crate::account::INSTANCE.reset().await;
    crate::post::INSTANCE.reset().await;
    crate::post::cache::INSTANCE.reset().await;
}

/// Create an account and verify it.
#[serial]
#[async_std::test]
async fn account_registry() {
    reset_all().await;

    let mut app = tide::new();
    app.at("/api/account/create")
        .post(crate::account::handle::create_account);
    app.at("/api/account/verify")
        .post(crate::account::handle::verify_account);

    {
        use sms3rs_shared::account::handle::AccountCreateDescriptor;

        let descriptor = AccountCreateDescriptor {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
        };

        let response_json: serde_json::Value = app
            .post("/api/account/create")
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
    }

    // Wrong verification code
    {
        use sms3rs_shared::account::handle::AccountVerifyDescriptor;

        let verification_code = 123456;
        let descriptor = AccountVerifyDescriptor {
            code: verification_code,
            variant: sms3rs_shared::account::handle::AccountVerifyVariant::Activate {
                email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                name: "Jiening Yu".to_string(),
                id: 2522320,
                phone: 16601550826,
                house: Some(sms3rs_shared::account::House::ZhiZhi),
                organization: None,
                password: "password123456".to_string(),
            },
        };

        let response_json: serde_json::Value = app
            .post("/api/account/verify")
            .body_json(&descriptor)
            .unwrap()
            .recv_json()
            .await
            .unwrap();

        assert_ne!(
            response_json
                .as_object()
                .unwrap()
                .get("status")
                .unwrap()
                .as_str()
                .unwrap(),
            "success"
        )
    }

    {
        use sms3rs_shared::account::handle::AccountVerifyDescriptor;

        let verification_code =
            crate::account::verify::VERIFICATION_CODE.load(std::sync::atomic::Ordering::Relaxed);
        let descriptor = AccountVerifyDescriptor {
            code: verification_code,
            variant: sms3rs_shared::account::handle::AccountVerifyVariant::Activate {
                email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                name: "Jiening Yu".to_string(),
                id: 2522320,
                phone: 16601550826,
                house: Some(sms3rs_shared::account::House::ZhiZhi),
                organization: None,
                password: "password123456".to_string(),
            },
        };

        let response_json: serde_json::Value = app
            .post("/api/account/verify")
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
        )
    }
}

/// Login, logout an account and test for `RequirePermissionContext`.
#[serial]
#[async_std::test]
async fn account_logging() {
    reset_all().await;

    let mut app = tide::new();
    app.at("/api/account/login")
        .post(crate::account::handle::login_account);
    app.at("/api/account/logout")
        .post(crate::account::handle::logout_account);
    app.at("/api/account/signout")
        .post(crate::account::handle::sign_out_account);

    let account_id = 123456;
    let password = "password123456";

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
                permissions: vec![],
                registration_time: chrono::Utc::now(),
                registration_ip: Some("127.0.0.1".to_string()),
                password_sha: digest(password.to_string()),
                token_expiration_time: 0,
            },
            tokens: crate::account::verify::Tokens::new(),
            verify: crate::account::UserVerifyVariant::None,
        })
        .await;

    let token;

    {
        use sms3rs_shared::account::handle::AccountLoginDescriptor;

        let descriptor = AccountLoginDescriptor {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
            password: password.to_string(),
        };

        let response_json: serde_json::Value = app
            .post("/api/account/login")
            .body_json(&descriptor)
            .unwrap()
            .recv_json()
            .await
            .unwrap();

        assert!(
            response_json
                .as_object()
                .unwrap()
                .get("status")
                .unwrap()
                .as_str()
                .unwrap()
                == "success"
        );

        assert_eq!(
            response_json
                .as_object()
                .unwrap()
                .get("account_id")
                .unwrap()
                .as_u64()
                .unwrap(),
            account_id
        );

        token = response_json
            .as_object()
            .unwrap()
            .get("token")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
    }

    {
        let cxt = crate::RequirePermissionContext {
            token: token.to_string(),
            user_id: account_id,
        };

        // Test `RequirePermissionContext`
        assert!(cxt.valid(vec![]).await.unwrap());
        assert!(!cxt
            .valid(vec![sms3rs_shared::account::Permission::OP])
            .await
            .unwrap());
    }

    {
        let response_json: serde_json::Value = app
            .post("/api/account/logout")
            .header("Token", token.to_string())
            .header("AccountId", account_id.to_string())
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

        let cxt = crate::RequirePermissionContext {
            token: token.to_string(),
            user_id: account_id,
        };
        assert!(!cxt.valid(vec![]).await.unwrap());
    }
}
