use super::*;

use serial_test::serial;
use sha256::digest;
use std::ops::Deref;

/// Test: create an account and verify it.
#[serial]
#[tokio::test]
async fn registry() {
    reset_all().await;

    let app = actix_web::test::init_service(
        actix_web::App::new()
            .service(crate::account::handle::create_account)
            .service(crate::account::handle::verify_account),
    )
    .await;

    assert_eq!(
        actix_web::test::call_service(
            &app,
            actix_web::test::TestRequest::post()
                .uri("/api/account/create")
                .param("email", "yujiening2025@i.pkuschool.edu.cn")
                .to_request(),
        )
        .await
        .status(),
        actix_web::http::StatusCode::OK
    );

    // Wrong verification code
    {
        use sms3rs_shared::account::handle::AccountVerifyDescriptor;

        let verification_code = crate::account::verify::VERIFICATION_CODE
            .load(std::sync::atomic::Ordering::Relaxed)
            - 1;
        let descriptor = AccountVerifyDescriptor {
            code: verification_code,
            variant: sms3rs_shared::account::handle::AccountVerifyVariant::Activate {
                name: "Jiening Yu".to_string(),
                id: 2522320,
                phone: 16601550826,
                house: Some(sms3rs_shared::account::House::ZhiZhi),
                organization: None,
                password: "password123456".to_string(),
            },
        };

        assert_ne!(
            actix_web::test::call_service(
                &app,
                actix_web::test::TestRequest::post()
                    .uri("/api/account/verify")
                    .param("email", "yujiening2025@i.pkuschool.edu.cn")
                    .set_json(descriptor)
                    .to_request(),
            )
            .await
            .status(),
            actix_web::http::StatusCode::OK
        );
    }

    {
        use sms3rs_shared::account::handle::AccountVerifyDescriptor;

        let verification_code =
            crate::account::verify::VERIFICATION_CODE.load(std::sync::atomic::Ordering::Relaxed);
        let descriptor = AccountVerifyDescriptor {
            code: verification_code,
            variant: sms3rs_shared::account::handle::AccountVerifyVariant::Activate {
                name: "Jiening Yu".to_string(),
                id: 2522320,
                phone: 16601550826,
                house: Some(sms3rs_shared::account::House::ZhiZhi),
                organization: None,
                password: "password123456".to_string(),
            },
        };

        assert_eq!(
            actix_web::test::call_service(
                &app,
                actix_web::test::TestRequest::post()
                    .uri("/api/account/verify")
                    .param("email", "yujiening2025@i.pkuschool.edu.cn")
                    .set_json(descriptor)
                    .to_request(),
            )
            .await
            .status(),
            actix_web::http::StatusCode::OK
        );
    }
}

/// Test for logging in an account.
#[serial]
#[tokio::test]
async fn login() {
    reset_all().await;

    let app = actix_web::test::init_service(
        actix_web::App::new().service(crate::account::handle::login_account),
    )
    .await;

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
        password: password.to_string(),
    };

    let response = actix_web::test::call_service(
        &app,
        actix_web::test::TestRequest::post()
            .uri("/api/account/login")
            .param("email", "yujiening2025@i.pkuschool.edu.cn")
            .set_json(descriptor)
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), actix_web::http::StatusCode::OK);

    let body_json = actix_web::test::read_body_json::<serde_json::Value, _>(response).await;

    assert_eq!(
        body_json
            .as_object()
            .unwrap()
            .get("account_id")
            .unwrap()
            .as_u64()
            .unwrap(),
        account_id
    );

    token = body_json
        .as_object()
        .unwrap()
        .get("token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let cxt = crate::RequirePermissionContext {
        token: token.to_string(),
        account_id,
    };

    assert!(cxt.valid(vec![]).await.unwrap());
}

/// Test for usage of `RequirePermissionContext`.
#[serial]
#[tokio::test]
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
            account_id,
        };

        assert!(cxt.valid(vec![]).await.unwrap());
        assert!(!cxt
            .valid(vec![sms3rs_shared::account::Permission::OP])
            .await
            .unwrap());

        let cxt_wrong = crate::RequirePermissionContext {
            token: "wrongtoken".to_string(),
            account_id,
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
            account_id,
        };

        assert!(!cxt.valid(vec![]).await.unwrap_or(true));
    }
}

/// Test for logging out an account.
#[serial]
#[tokio::test]
async fn logout() {
    reset_all().await;

    let app = actix_web::test::init_service(
        actix_web::App::new().service(crate::account::handle::logout_account),
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
                name: "Yu Jiening".to_string(),
                school_id: 2522320,
                house: Some(sms3rs_shared::account::House::ZhiZhi),
                phone: 16601550826,
                organization: None,
                permissions: vec![],
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
        })
        .await;

    assert_eq!(
        actix_web::test::call_service(
            &app,
            actix_web::test::TestRequest::post()
                .uri("/api/account/logout")
                .insert_header(crate::RequirePermissionContext {
                    account_id,
                    token: token.to_string()
                })
                .to_request(),
        )
        .await
        .status(),
        actix_web::http::StatusCode::OK
    );

    let cxt = crate::RequirePermissionContext {
        token: token.to_string(),
        account_id,
    };
    assert!(!cxt.valid(vec![]).await.unwrap());
}

#[serial]
#[tokio::test]
async fn signout() {
    reset_all().await;

    let app = actix_web::test::init_service(
        actix_web::App::new().service(crate::account::handle::sign_out_account),
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
                permissions: vec![],
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
        })
        .await;

    use sms3rs_shared::account::handle::AccountSignOutDescriptor;

    assert_ne!(
        actix_web::test::call_service(
            &app,
            actix_web::test::TestRequest::post()
                .uri("/api/account/signout")
                .insert_header(crate::RequirePermissionContext {
                    account_id,
                    token: token.to_string()
                })
                .set_json(AccountSignOutDescriptor {
                    password: "fakepassword".to_string(),
                })
                .to_request(),
        )
        .await
        .status(),
        actix_web::http::StatusCode::OK
    );

    assert_eq!(
        actix_web::test::call_service(
            &app,
            actix_web::test::TestRequest::post()
                .uri("/api/account/signout")
                .insert_header(crate::RequirePermissionContext {
                    account_id,
                    token: token.to_string()
                })
                .set_json(AccountSignOutDescriptor {
                    password: password.to_string()
                })
                .to_request(),
        )
        .await
        .status(),
        actix_web::http::StatusCode::OK
    );

    assert!(crate::account::INSTANCE.inner().read().await.is_empty());
    assert!(crate::account::INSTANCE.index().read().await.is_empty());
}

#[serial]
#[tokio::test]
async fn view() {
    reset_all().await;

    let app = actix_web::test::init_service(
        actix_web::App::new().service(crate::account::handle::view_account),
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
                permissions: vec![],
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
        })
        .await;

    use sms3rs_shared::account::handle::ViewAccountResult;

    let response = actix_web::test::call_service(
        &app,
        actix_web::test::TestRequest::post()
            .uri("/api/account/view")
            .insert_header(crate::RequirePermissionContext {
                account_id,
                token: token.to_string(),
            })
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), actix_web::http::StatusCode::OK);

    let result: Result<ViewAccountResult, _> = serde_json::from_value(
        actix_web::test::read_body_json::<serde_json::Value, _>(response)
            .await
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
#[tokio::test]
async fn edit() {
    reset_all().await;

    let app = actix_web::test::init_service(
        actix_web::App::new().service(crate::account::handle::edit_account),
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
                permissions: vec![],
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
        })
        .await;

    use sms3rs_shared::account::handle::{AccountEditDescriptor, AccountEditVariant};

    assert_ne!(
        actix_web::test::call_service(
            &app,
            actix_web::test::TestRequest::post()
                .uri("/api/account/edit")
                .insert_header(crate::RequirePermissionContext {
                    account_id,
                    token: token.to_string()
                })
                .set_json(AccountEditDescriptor {
                    variants: vec![AccountEditVariant::Password {
                        old: "1".to_string(),
                        new: "pass".to_string(),
                    }],
                })
                .to_request(),
        )
        .await
        .status(),
        actix_web::http::StatusCode::OK
    );

    assert_eq!(
        actix_web::test::call_service(
            &app,
            actix_web::test::TestRequest::post()
                .uri("/api/account/edit")
                .insert_header(crate::RequirePermissionContext {
                    account_id,
                    token: token.to_string()
                })
                .set_json(AccountEditDescriptor {
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
                })
                .to_request(),
        )
        .await
        .status(),
        actix_web::http::StatusCode::OK
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
#[tokio::test]
async fn reset_password() {
    reset_all().await;

    let app = actix_web::test::init_service(
        actix_web::App::new()
            .service(crate::account::handle::reset_password)
            .service(crate::account::handle::verify_account),
    )
    .await;

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
                password_sha: digest(password.to_string()),
                token_expiration_time: 0,
            },
            tokens: crate::account::verify::Tokens::new(),
            verify: crate::account::UserVerifyVariant::None,
        })
        .await;

    assert_eq!(
        actix_web::test::call_service(
            &app,
            actix_web::test::TestRequest::post()
                .uri("/api/account/reset-password")
                .param("email", "yujiening2025@i.pkuschool.edu.cn")
                .to_request(),
        )
        .await
        .status(),
        actix_web::http::StatusCode::OK
    );

    {
        use sms3rs_shared::account::handle::{AccountVerifyDescriptor, AccountVerifyVariant};

        // Wrong verification code
        assert_ne!(
            actix_web::test::call_service(
                &app,
                actix_web::test::TestRequest::post()
                    .uri("/api/account/verify")
                    .param("email", "yujiening2025@i.pkuschool.edu.cn")
                    .set_json(AccountVerifyDescriptor {
                        code: crate::account::verify::VERIFICATION_CODE
                            .load(std::sync::atomic::Ordering::Relaxed)
                            - 1,
                        variant: AccountVerifyVariant::ResetPassword(new_password.to_string()),
                    })
                    .to_request(),
            )
            .await
            .status(),
            actix_web::http::StatusCode::OK
        );

        assert_eq!(
            actix_web::test::call_service(
                &app,
                actix_web::test::TestRequest::post()
                    .uri("/api/account/verify")
                    .param("email", "yujiening2025@i.pkuschool.edu.cn")
                    .set_json(AccountVerifyDescriptor {
                        code: crate::account::verify::VERIFICATION_CODE
                            .load(std::sync::atomic::Ordering::Relaxed),
                        variant: AccountVerifyVariant::ResetPassword(new_password.to_string()),
                    })
                    .to_request(),
            )
            .await
            .status(),
            actix_web::http::StatusCode::OK
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
            crate::account::Account::Verified { attributes, .. } => {
                assert_eq!(attributes.password_sha, digest(new_password.to_string()))
            }
            _ => unreachable!(),
        }
    }
}
