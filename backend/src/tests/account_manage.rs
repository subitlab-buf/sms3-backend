use std::ops::Deref;

use super::*;

use axum::http;
use hyper::{Request, StatusCode};
use serial_test::serial;
use sha256::digest;
use tower::ServiceExt;

#[serial]
#[tokio::test]
async fn make() {
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
            house: Some(sms3rs_shared::account::House::ZhiZhi),
            phone: 16601550826,
            organization: None,
            permissions: vec![sms3rs_shared::account::Permission::ManageAccounts],
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

    use sms3rs_shared::account::handle::manage::MakeAccountDescriptor;

    let descriptor = MakeAccountDescriptor {
        email: lettre::Address::new("myg", "i.pkuschool.edu.cn").unwrap(),
        name: "Yuguo Ma".to_string(),
        school_id: 114514,
        phone: 1919810,
        house: None,
        organization: Some("PKU".to_string()),
        password: "password".to_string(),
        permissions: vec![
            sms3rs_shared::account::Permission::ManageAccounts,
            sms3rs_shared::account::Permission::Op,
        ],
    };

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/account/manage/create")
                .method("POST")
                .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .header("Token", &token)
                .header("AccountId", account_id)
                .body(serde_json::to_vec(&descriptor).unwrap().into())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_json: serde_json::Value =
        serde_json::from_slice(&hyper::body::to_bytes(response.into_body()).await.unwrap())
            .unwrap();

    let instance = crate::account::INSTANCE.inner().read();

    let a = instance.get(
        *crate::account::INSTANCE
            .index()
            .get(
                &response_json
                    .as_object()
                    .unwrap()
                    .get("account_id")
                    .unwrap()
                    .as_u64()
                    .unwrap(),
            )
            .unwrap()
            .value(),
    );

    assert!(a.is_some());

    assert!(a
        .unwrap()
        .read()
        .has_permission(sms3rs_shared::account::Permission::ManageAccounts));

    // test for permission overflowing
    assert!(!a
        .unwrap()
        .read()
        .has_permission(sms3rs_shared::account::Permission::Op));
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
            house: Some(sms3rs_shared::account::House::ZhiZhi),
            phone: 16601550826,
            organization: None,
            permissions: vec![sms3rs_shared::account::Permission::ViewAccounts],
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

    let test_account_id_0 = 114514;
    let test_account_id_1 = 114513;
    let test_password = "password123456";

    crate::account::INSTANCE.push(crate::account::Account::Verified {
        id: test_account_id_0,
        attributes: crate::account::UserAttributes {
            email: lettre::Address::new("myg", "i.pkuschool.edu.cn").unwrap(),
            name: "Yuguo Ma".to_string(),
            school_id: 114514,
            house: None,
            phone: 1919810,
            organization: None,
            permissions: vec![sms3rs_shared::account::Permission::Op],
            registration_time: chrono::Utc::now(),
            password_sha: digest(test_password.to_string()),
            token_expiration_time: 0,
        },
        tokens: crate::account::verify::Tokens::new(),
        verify: crate::account::UserVerifyVariant::None,
    });

    crate::account::INSTANCE.push(crate::account::Account::Verified {
        id: test_account_id_1,
        attributes: crate::account::UserAttributes {
            email: lettre::Address::new("myg", "i.pkuschool.edu.cn").unwrap(),
            name: "Yuguo Ma".to_string(),
            school_id: 114514,
            house: None,
            phone: 1919810,
            organization: None,
            permissions: vec![],
            registration_time: chrono::Utc::now(),
            password_sha: digest(test_password.to_string()),
            token_expiration_time: 0,
        },
        tokens: crate::account::verify::Tokens::new(),
        verify: crate::account::UserVerifyVariant::None,
    });

    let descriptor = sms3rs_shared::account::handle::manage::ViewAccountDescriptor {
        accounts: vec![test_account_id_0, test_account_id_1],
    };

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/account/manage/view")
                .method("POST")
                .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .header("Token", &token)
                .header("AccountId", account_id)
                .body(serde_json::to_vec(&descriptor).unwrap().into())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_json: serde_json::Value =
        serde_json::from_slice(&hyper::body::to_bytes(response.into_body()).await.unwrap())
            .unwrap();

    let result: Vec<sms3rs_shared::account::handle::manage::ViewAccountResult> = response_json
        .as_object()
        .unwrap()
        .get("results")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|e| serde_json::from_value(e.clone()).unwrap())
        .collect();

    assert!(matches!(
        result[0],
        sms3rs_shared::account::handle::manage::ViewAccountResult::Err { .. }
    ));

    assert!(matches!(
        result[1],
        sms3rs_shared::account::handle::manage::ViewAccountResult::Ok(_)
    ));

    if let sms3rs_shared::account::handle::manage::ViewAccountResult::Err { id, .. } = &result[0] {
        assert_eq!(id, &test_account_id_0)
    } else {
        unreachable!()
    }

    if let sms3rs_shared::account::handle::manage::ViewAccountResult::Ok(e) = &result[1] {
        assert_eq!(e.id, test_account_id_1)
    } else {
        unreachable!()
    }
}

#[serial]
#[tokio::test]
async fn modify() {
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
            house: Some(sms3rs_shared::account::House::ZhiZhi),
            phone: 16601550826,
            organization: None,
            permissions: vec![sms3rs_shared::account::Permission::ManageAccounts],
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

    let test_account_id_0 = 114514;
    let test_account_id_1 = 114513;
    let test_password = "password123456";

    crate::account::INSTANCE.push(crate::account::Account::Verified {
        id: test_account_id_0,
        attributes: crate::account::UserAttributes {
            email: lettre::Address::new("myg", "i.pkuschool.edu.cn").unwrap(),
            name: "Yuguo Ma".to_string(),
            school_id: 114514,
            house: None,
            phone: 1919810,
            organization: None,
            permissions: vec![sms3rs_shared::account::Permission::Op],
            registration_time: chrono::Utc::now(),
            password_sha: digest(test_password.to_string()),
            token_expiration_time: 0,
        },
        tokens: crate::account::verify::Tokens::new(),
        verify: crate::account::UserVerifyVariant::None,
    });

    crate::account::INSTANCE.push(crate::account::Account::Verified {
        id: test_account_id_1,
        attributes: crate::account::UserAttributes {
            email: lettre::Address::new("myg", "i.pkuschool.edu.cn").unwrap(),
            name: "Yuguo Ma".to_string(),
            school_id: 114514,
            house: None,
            phone: 1919810,
            organization: None,
            permissions: vec![],
            registration_time: chrono::Utc::now(),
            password_sha: digest(test_password.to_string()),
            token_expiration_time: 0,
        },
        tokens: crate::account::verify::Tokens::new(),
        verify: crate::account::UserVerifyVariant::None,
    });

    use sms3rs_shared::account::{
        handle::manage::{AccountModifyDescriptor, AccountModifyVariant},
        Permission,
    };

    {
        let descriptor = AccountModifyDescriptor {
            account_id: test_account_id_0,
            variants: vec![AccountModifyVariant::Name("Tianyang He".to_string())],
        };

        assert_ne!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/account/modify")
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

    {
        let descriptor = AccountModifyDescriptor {
            account_id: test_account_id_1,
            variants: vec![
                AccountModifyVariant::Email(
                    lettre::Address::new("hetianyang2021", "i.pkuschool.edu.cn").unwrap(),
                ),
                AccountModifyVariant::Name("Tianyang He".to_string()),
                AccountModifyVariant::SchoolId(2100000),
                AccountModifyVariant::Phone(1),
                AccountModifyVariant::House(Some(sms3rs_shared::account::House::ZhengXin)),
                AccountModifyVariant::Organization(Some("SubIT".to_string())),
                AccountModifyVariant::Permission(vec![Permission::ManageAccounts, Permission::Op]),
            ],
        };

        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/account/manage/modify")
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

        let am = crate::account::INSTANCE.inner().read();

        let a = am
            .get(
                *crate::account::INSTANCE
                    .index()
                    .get(&test_account_id_1)
                    .unwrap()
                    .value(),
            )
            .unwrap()
            .read();

        assert!(matches!(
            a.deref(),
            crate::account::Account::Verified { .. }
        ));

        if let crate::account::Account::Verified { id, attributes, .. } = a.deref() {
            assert_eq!(*id, test_account_id_1);

            assert_eq!(
                attributes.email,
                lettre::Address::new("hetianyang2021", "i.pkuschool.edu.cn").unwrap()
            );
            assert_eq!(attributes.name, "Tianyang He");
            assert_eq!(attributes.school_id, 2100000);
            assert_eq!(attributes.phone, 1);
            assert_eq!(
                attributes.house,
                Some(sms3rs_shared::account::House::ZhengXin)
            );
            assert_eq!(attributes.organization, Some("SubIT".to_string()));
            assert_eq!(attributes.permissions, &[Permission::ManageAccounts]);
        } else {
            unreachable!()
        }
    }
}
