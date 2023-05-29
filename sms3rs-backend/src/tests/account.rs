use super::*;

use serial_test::serial;
use sha256::digest;
use std::ops::Deref;
use tide_testing::TideTestingExt;

/// Test: create an account and verify it.
#[serial]
#[async_std::test]
async fn registry() {
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

        let verification_code = crate::account::verify::VERIFICATION_CODE
            .load(std::sync::atomic::Ordering::Relaxed)
            - 1;
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

/// Test for logging in an account.
#[serial]
#[async_std::test]
async fn login() {
    reset_all().await;

    let mut app = tide::new();
    app.at("/api/account/login")
        .post(crate::account::handle::login_account);

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

    let cxt = crate::RequirePermissionContext {
        token: token.to_string(),
        account_id: account_id,
    };

    assert!(cxt.valid(vec![]).await.unwrap());
}

/// Test for usage of `RequirePermissionContext`.
#[serial]
#[async_std::test]
async fn require_permission_context() {
    reset_all().await;

    let account_id = 123456;

    {
        let password = "password123456";
        let token;

        crate::account::INSTANCE
            .push(crate::account::Account::Verified {
                id: account_id,
                attributes: crate::account::UserAttributes {
                    email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                    name: "Yu Jiening".to_string(),
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
                tokens: {
                    let mut t = crate::account::verify::Tokens::new();
                    token = t.new_token(account_id, 0);
                    t
                },
                verify: crate::account::UserVerifyVariant::None,
            })
            .await;

        let cxt = crate::RequirePermissionContext {
            token: token.to_string(),
            account_id: account_id,
        };

        assert!(cxt.valid(vec![]).await.unwrap());
        assert!(!cxt
            .valid(vec![sms3rs_shared::account::Permission::OP])
            .await
            .unwrap());

        let cxt_wrong = crate::RequirePermissionContext {
            token: "wrongtoken".to_string(),
            account_id: account_id,
        };

        assert!(!cxt_wrong.valid(vec![]).await.unwrap());
    }

    {
        crate::account::INSTANCE
            .push(crate::account::Account::Unverified(
                crate::account::verify::Context {
                    email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                    expire_time: (chrono::Utc::now() + chrono::Days::new(1)).naive_utc(),
                    code: 6,
                },
            ))
            .await;

        let cxt = crate::RequirePermissionContext {
            token: 6.to_string(),
            account_id: account_id,
        };

        assert!(!cxt.valid(vec![]).await.unwrap_or(true));
    }
}

/// Test for logging out an account.
#[serial]
#[async_std::test]
async fn logout() {
    reset_all().await;

    let mut app = tide::new();
    app.at("/api/account/logout")
        .post(crate::account::handle::logout_account);

    let account_id = 123456;
    let password = "password123456";

    let token;

    crate::account::INSTANCE
        .push(crate::account::Account::Verified {
            id: account_id,
            attributes: crate::account::UserAttributes {
                email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                name: "Yu Jiening".to_string(),
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
            tokens: {
                let mut t = crate::account::verify::Tokens::new();
                token = t.new_token(account_id, 0);
                t
            },
            verify: crate::account::UserVerifyVariant::None,
        })
        .await;

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
        account_id: account_id,
    };
    assert!(!cxt.valid(vec![]).await.unwrap());
}

#[serial]
#[async_std::test]
async fn signout() {
    reset_all().await;
    let mut app = tide::new();
    app.at("/api/account/signout")
        .post(crate::account::handle::sign_out_account);

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
                permissions: vec![],
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

    use sms3rs_shared::account::handle::AccountSignOutDescriptor;

    {
        let descriptor_wrong = AccountSignOutDescriptor {
            password: "fakepassword".to_string(),
        };

        let response_json: serde_json::Value = app
            .post("/api/account/signout")
            .header("Token", token.to_string())
            .header("AccountId", account_id.to_string())
            .body_json(&descriptor_wrong)
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
        );
    }

    {
        let descriptor = AccountSignOutDescriptor {
            password: password.to_string(),
        };

        let response_json: serde_json::Value = app
            .post("/api/account/signout")
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
    }

    assert!(crate::account::INSTANCE.inner().read().await.is_empty());
    assert!(crate::account::INSTANCE.index().read().await.is_empty());
}

#[serial]
#[async_std::test]
async fn view() {
    reset_all().await;
    let mut app = tide::new();
    app.at("/api/account/view")
        .post(crate::account::handle::view_account);

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
                permissions: vec![],
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

    use sms3rs_shared::account::handle::ViewAccountResult;

    let response_json: serde_json::Value = app
        .post("/api/account/view")
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

    let result: Result<ViewAccountResult, _> = serde_json::from_value(
        response_json
            .as_object()
            .unwrap()
            .get("result")
            .unwrap()
            .clone(),
    );

    assert!(result.is_ok());
    assert_eq!(result.unwrap().id, account_id);
}

#[serial]
#[async_std::test]
async fn edit() {
    reset_all().await;
    let mut app = tide::new();
    app.at("/api/account/edit")
        .post(crate::account::handle::edit_account);

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
                permissions: vec![],
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

    use sms3rs_shared::account::handle::{AccountEditDescriptor, AccountEditVariant};

    {
        let descriptor_wrong_pass = AccountEditDescriptor {
            variants: vec![AccountEditVariant::Password {
                old: "1".to_string(),
                new: "pass".to_string(),
            }],
        };

        let response_json: serde_json::Value = app
            .post("/api/account/edit")
            .header("Token", token.to_string())
            .header("AccountId", account_id.to_string())
            .body_json(&descriptor_wrong_pass)
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
        );
    }

    let descriptor = AccountEditDescriptor {
        variants: vec![
            AccountEditVariant::Name("Tianyang He".to_string()),
            AccountEditVariant::SchoolId(2100000),
            AccountEditVariant::Phone(114514),
            AccountEditVariant::House(Some(sms3rs_shared::account::House::ZhengXin)),
            AccountEditVariant::Organization(Some("SubIT".to_string())),
            AccountEditVariant::Password {
                old: password.to_string(),
                new: "newpassword".to_string(),
            },
            AccountEditVariant::TokenExpireTime(9),
        ],
    };

    let response_json: serde_json::Value = app
        .post("/api/account/edit")
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
    let a = instance
        .get(
            *crate::account::INSTANCE
                .index()
                .read()
                .await
                .get(&account_id)
                .unwrap(),
        )
        .unwrap();
    let ar = a.read().await;

    assert!(matches!(
        ar.deref(),
        crate::account::Account::Verified { .. }
    ));

    match ar.deref() {
        crate::account::Account::Verified { id, attributes, .. } => {
            assert_eq!(*id, account_id);
            assert_eq!(
                attributes.email,
                lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
            );

            assert_eq!(&attributes.name, "Tianyang He");
            assert_eq!(attributes.school_id, 2100000);
            assert_eq!(attributes.phone, 114514);
            assert_eq!(
                attributes.house,
                Some(sms3rs_shared::account::House::ZhengXin)
            );
            assert_eq!(attributes.organization, Some("SubIT".to_string()));
            assert_eq!(attributes.password_sha, digest("newpassword").to_string());
            assert_eq!(attributes.token_expiration_time, 9);
        }
        _ => unreachable!(),
    }
}

#[serial]
#[async_std::test]
async fn reset_password() {
    reset_all().await;
    let mut app = tide::new();
    app.at("/api/account/reset-password")
        .post(crate::account::handle::reset_password);
    app.at("/api/account/verify")
        .post(crate::account::handle::verify_account);

    let account_id = 123456;
    let password = "password123456";
    let new_password = "newpassword";

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

    {
        use sms3rs_shared::account::handle::ResetPasswordDescriptor;

        let descriptor = ResetPasswordDescriptor {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
        };

        let response_json: serde_json::Value = app
            .post("/api/account/reset-password")
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

    {
        use sms3rs_shared::account::handle::{AccountVerifyDescriptor, AccountVerifyVariant};

        // Wrong verification code
        {
            let verification_code = crate::account::verify::VERIFICATION_CODE
                .load(std::sync::atomic::Ordering::Relaxed)
                - 1;
            let descriptor = AccountVerifyDescriptor {
                code: verification_code,
                variant: AccountVerifyVariant::ResetPassword {
                    email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                    password: new_password.to_string(),
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
            );
        }

        {
            let verification_code = crate::account::verify::VERIFICATION_CODE
                .load(std::sync::atomic::Ordering::Relaxed);
            let descriptor = AccountVerifyDescriptor {
                code: verification_code,
                variant: AccountVerifyVariant::ResetPassword {
                    email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                    password: new_password.to_string(),
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
            );
        }

        let instance = crate::account::INSTANCE.inner().read().await;
        let a = instance
            .get(
                *crate::account::INSTANCE
                    .index()
                    .read()
                    .await
                    .get(&account_id)
                    .unwrap(),
            )
            .unwrap();
        let ar = a.read().await;

        assert!(matches!(
            ar.deref(),
            crate::account::Account::Verified { .. }
        ));

        match ar.deref() {
            crate::account::Account::Verified { attributes, .. } => {
                assert_eq!(attributes.password_sha, digest(new_password.to_string()))
            }
            _ => unreachable!(),
        }
    }
}
