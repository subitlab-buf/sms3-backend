use super::*;

use axum::http::{self, Request, StatusCode};
use serial_test::serial;
use sha256::digest;
use std::ops::Deref;

use tower::util::ServiceExt;

/// Test: create an account and verify it.
#[serial]
#[tokio::test]
async fn registry() {
    reset_all();

    let app = crate::router();

    {
        use sms3_shared::account::handle::AccountCreateDescriptor;

        let descriptor = AccountCreateDescriptor {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
        };

        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/account/create")
                        .method("POST")
                        .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                        .body(serde_json::to_vec(&descriptor).unwrap().into())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }

    // Wrong verification code
    {
        use sms3_shared::account::handle::AccountVerifyDescriptor;

        let verification_code = crate::account::verify::VERIFICATION_CODE
            .load(std::sync::atomic::Ordering::Relaxed)
            - 1;

        let descriptor = AccountVerifyDescriptor {
            code: verification_code,
            variant: sms3_shared::account::handle::AccountVerifyVariant::Activate {
                email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                name: "Jiening Yu".to_string(),
                id: 2522320,
                phone: 16601550826,
                house: Some(sms3_shared::account::House::ZhiZhi),
                organization: None,
                password: "password123456".to_string(),
            },
        };

        assert_ne!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/account/verify")
                        .method("POST")
                        .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                        .body(serde_json::to_vec(&descriptor).unwrap().into())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }

    {
        use sms3_shared::account::handle::AccountVerifyDescriptor;

        let verification_code =
            crate::account::verify::VERIFICATION_CODE.load(std::sync::atomic::Ordering::Relaxed);

        let descriptor = AccountVerifyDescriptor {
            code: verification_code,
            variant: sms3_shared::account::handle::AccountVerifyVariant::Activate {
                email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                name: "Jiening Yu".to_string(),
                id: 2522320,
                phone: 16601550826,
                house: Some(sms3_shared::account::House::ZhiZhi),
                organization: None,
                password: "password123456".to_string(),
            },
        };

        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/account/verify")
                        .method("POST")
                        .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                        .body(serde_json::to_vec(&descriptor).unwrap().into())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
}

/// Test for logging in an account.
#[serial]
#[tokio::test]
async fn login() {
    reset_all();

    let app = crate::router();

    let account_id = 123456;
    let password = "password123456";

    crate::account::INSTANCE.push(crate::account::Account::Verified {
        id: account_id,
        attributes: crate::account::UserAttributes {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
            name: "Jiening Yu".to_string(),
            school_id: 2522320,
            house: Some(sms3_shared::account::House::ZhiZhi),
            phone: 16601550826,
            organization: None,
            permissions: vec![],
            registration_time: chrono::Utc::now(),
            password_sha: digest(password.to_string()),
            token_expiration_time: 0,
        },
        tokens: crate::account::verify::Tokens::new(),
        verify: crate::account::UserVerifyVariant::None,
    });

    use sms3_shared::account::handle::AccountLoginDescriptor;

    let descriptor = AccountLoginDescriptor {
        email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
        password: password.to_string(),
    };

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/account/login")
                .method("POST")
                .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(serde_json::to_vec(&descriptor).unwrap().into())
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
            .get("account_id")
            .unwrap()
            .as_u64()
            .unwrap(),
        account_id
    );

    let token = response_json
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

    assert!(cxt.try_valid(&[]).unwrap());
}

/// Test for usage of `RequirePermissionContext`.
#[serial]
#[test]
fn require_permission_context() {
    reset_all();

    let account_id = 123456;

    {
        let password = "password123456";
        let token;

        crate::account::INSTANCE.push(crate::account::Account::Verified {
            id: account_id,
            attributes: crate::account::UserAttributes {
                email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                name: "Yu Jiening".to_string(),
                school_id: 2522320,
                house: Some(sms3_shared::account::House::ZhiZhi),
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
        });

        let cxt = crate::RequirePermissionContext {
            token: token.to_string(),
            account_id,
        };

        assert!(cxt.try_valid(&[]).unwrap());

        assert!(!cxt
            .try_valid(&[sms3_shared::account::Permission::Op])
            .unwrap());

        let ctx_wrong = crate::RequirePermissionContext {
            token: "wrongtoken".to_string(),
            account_id,
        };

        assert!(!ctx_wrong.try_valid(&[]).unwrap());
    }

    {
        crate::account::INSTANCE.push(crate::account::Account::Unverified(
            crate::account::verify::Context {
                email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
                expire_time: (chrono::Utc::now() + chrono::Days::new(1)).naive_utc(),
                code: 6,
            },
        ));

        let ctx = crate::RequirePermissionContext {
            token: 6.to_string(),
            account_id,
        };

        assert!(!ctx.try_valid(&[]).unwrap_or(true));
    }
}

/// Test for logging out an account.
#[serial]
#[tokio::test]
async fn logout() {
    reset_all();

    let app = crate::router();

    let account_id = 123456;
    let password = "password123456";

    let token;

    crate::account::INSTANCE.push(crate::account::Account::Verified {
        id: account_id,
        attributes: crate::account::UserAttributes {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
            name: "Yu Jiening".to_string(),
            school_id: 2522320,
            house: Some(sms3_shared::account::House::ZhiZhi),
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
    });

    assert_eq!(
        app.oneshot(
            Request::builder()
                .uri("/api/account/logout")
                .method("POST")
                .header("Token", &token)
                .header("AccountId", account_id)
                .body(hyper::Body::empty())
                .unwrap()
        )
        .await
        .unwrap()
        .status(),
        StatusCode::OK
    );

    let ctx = crate::RequirePermissionContext {
        token: token.to_string(),
        account_id,
    };

    assert!(!ctx.try_valid(&[]).unwrap());
}

#[serial]
#[tokio::test]
async fn signout() {
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
    });

    use sms3_shared::account::handle::AccountSignOutDescriptor;

    {
        let descriptor_wrong = AccountSignOutDescriptor {
            password: "fakepassword".to_string(),
        };

        assert_ne!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/account/signout")
                        .method("POST")
                        .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                        .header("Token", &token)
                        .header("AccountId", account_id)
                        .body(serde_json::to_vec(&descriptor_wrong).unwrap().into())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }

    {
        let descriptor = AccountSignOutDescriptor {
            password: password.to_string(),
        };

        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/account/signout")
                        .method("POST")
                        .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                        .header("Token", &token)
                        .header("AccountId", account_id)
                        .body(serde_json::to_vec(&descriptor).unwrap().into())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }

    assert!(crate::account::INSTANCE.inner().read().is_empty());
    assert!(crate::account::INSTANCE.index().is_empty());
}

#[serial]
#[tokio::test]
async fn view() {
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
    });

    use sms3_shared::account::handle::ViewAccountResult;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/account/view")
                .method("POST")
                .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .header("Token", &token)
                .header("AccountId", account_id)
                .body(hyper::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let result: ViewAccountResult =
        serde_json::from_slice(&hyper::body::to_bytes(response.into_body()).await.unwrap())
            .unwrap();

    assert_eq!(result.id, account_id);
}

#[serial]
#[tokio::test]
async fn edit() {
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
    });

    use sms3_shared::account::handle::{AccountEditDescriptor, AccountEditVariant};

    {
        let descriptor_wrong_pass = AccountEditDescriptor {
            variants: vec![AccountEditVariant::Password {
                old: "1".to_string(),
                new: "pass".to_string(),
            }],
        };

        assert_ne!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/account/edit")
                        .method("POST")
                        .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                        .header("Token", &token)
                        .header("AccountId", account_id)
                        .body(serde_json::to_vec(&descriptor_wrong_pass).unwrap().into())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }

    let descriptor = AccountEditDescriptor {
        variants: vec![
            AccountEditVariant::Name("Tianyang He".to_string()),
            AccountEditVariant::SchoolId(2100000),
            AccountEditVariant::Phone(114514),
            AccountEditVariant::House(Some(sms3_shared::account::House::ZhengXin)),
            AccountEditVariant::Organization(Some("SubIT".to_string())),
            AccountEditVariant::Password {
                old: password.to_string(),
                new: "newpassword".to_string(),
            },
            AccountEditVariant::TokenExpireTime(9),
        ],
    };

    assert_eq!(
        app.oneshot(
            Request::builder()
                .uri("/api/account/edit")
                .method("POST")
                .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .header("Token", &token)
                .header("AccountId", account_id)
                .body(serde_json::to_vec(&descriptor).unwrap().into())
                .unwrap()
        )
        .await
        .unwrap()
        .status(),
        StatusCode::OK
    );

    let instance = crate::account::INSTANCE.inner().read();

    let a = instance
        .get(
            *crate::account::INSTANCE
                .index()
                .get(&account_id)
                .unwrap()
                .value(),
        )
        .unwrap();

    let ar = a.read();

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
                Some(sms3_shared::account::House::ZhengXin)
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
    reset_all();

    let app = crate::router();

    let account_id = 123456;
    let password = "password123456";
    let new_password = "newpassword";

    crate::account::INSTANCE.push(crate::account::Account::Verified {
        id: account_id,
        attributes: crate::account::UserAttributes {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
            name: "Jiening Yu".to_string(),
            school_id: 2522320,
            house: Some(sms3_shared::account::House::ZhiZhi),
            phone: 16601550826,
            organization: None,
            permissions: vec![],
            registration_time: chrono::Utc::now(),
            password_sha: digest(password.to_string()),
            token_expiration_time: 0,
        },
        tokens: crate::account::verify::Tokens::new(),
        verify: crate::account::UserVerifyVariant::None,
    });

    {
        use sms3_shared::account::handle::ResetPasswordDescriptor;

        let descriptor = ResetPasswordDescriptor {
            email: lettre::Address::new("yujiening2025", "i.pkuschool.edu.cn").unwrap(),
        };

        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/account/reset-password")
                        .method("POST")
                        .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                        .body(serde_json::to_vec(&descriptor).unwrap().into())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }

    {
        use sms3_shared::account::handle::{AccountVerifyDescriptor, AccountVerifyVariant};

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

            assert_ne!(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/account/verify")
                            .method("POST")
                            .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                            .body(serde_json::to_vec(&descriptor).unwrap().into())
                            .unwrap()
                    )
                    .await
                    .unwrap()
                    .status(),
                StatusCode::OK
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

            assert_eq!(
                app.oneshot(
                    Request::builder()
                        .uri("/api/account/verify")
                        .method("POST")
                        .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                        .body(serde_json::to_vec(&descriptor).unwrap().into())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
                StatusCode::OK
            );
        }

        let instance = crate::account::INSTANCE.inner().read();

        let a = instance
            .get(
                *crate::account::INSTANCE
                    .index()
                    .get(&account_id)
                    .unwrap()
                    .value(),
            )
            .unwrap();

        let ar = a.read();

        assert!(matches!(
            ar.deref(),
            crate::account::Account::Verified { .. }
        ));

        if let crate::account::Account::Verified { attributes, .. } = ar.deref() {
            assert_eq!(attributes.password_sha, digest(new_password.to_string()))
        }
    }
}
