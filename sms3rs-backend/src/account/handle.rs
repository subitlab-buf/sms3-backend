use std::ops::{Deref, DerefMut};

use rand::Rng;
use sms3rs_shared::account::handle::*;

/// Create an unverified account.
/// POST only.
///
/// Url: `/api/account/create/?email={email}`
#[actix_web::post("/api/account/create")]
pub async fn create_account(
    actix_web::web::Query(EmailTarget { email }): actix_web::web::Query<EmailTarget>,
) -> impl actix_web::Responder {
    let account_manager = &super::INSTANCE;

    for account in account_manager.inner().read().await.iter() {
        if account.read().await.email() == &email {
            return (
                format!("Account with email address {email} already exists!"),
                actix_web::http::StatusCode::CONFLICT,
            );
        }
    }

    let len = account_manager.inner().read().await.len();
    account_manager
        .inner()
        .write()
        .await
        .push(tokio::sync::RwLock::new({
            let account = match crate::account::Account::new(email).await {
                Ok(e) => e,
                Err(err) => return (err.to_string(), actix_web::http::StatusCode::OK),
            };

            account_manager
                .index()
                .write()
                .await
                .insert(account.id(), len);

            if !account.save().await {
                tracing::error!("Error while saving account {}", account.email());
            }

            account
        }));

    (String::new(), actix_web::http::StatusCode::OK)
}

/// Verify an account.
///
/// Url: `/api/account/verify/?email={email}`
///
/// Request body: See [`AccountVerifyDescriptor`]. (json)
///
/// Response: `200` with `account_id` (type: number) in json.
#[actix_web::post("/api/account/verify/{email}")]
pub async fn verify_account(
    actix_web::web::Query(EmailTarget { email }): actix_web::web::Query<EmailTarget>,
    descriptor: actix_web::web::Json<AccountVerifyDescriptor>,
) -> impl actix_web::Responder {
    let account_manager = &super::INSTANCE;

    for account in account_manager.inner().read().await.iter() {
        match &descriptor.variant {
            AccountVerifyVariant::Activate {
                name,
                id,
                phone,
                house,
                organization,
                password,
            } => {
                if {
                    let a = account.read().await;
                    if a.email() == &email {
                        let id = a.id();
                        drop(a);
                        account_manager.refresh(id).await;
                        true
                    } else {
                        false
                    }
                } {
                    let mut a = account.write().await;
                    if let Err(err) = a.verify(
                        descriptor.code,
                        super::AccountVerifyVariant::Activate(crate::account::UserAttributes {
                            email,
                            name: name.clone(),
                            school_id: *id,
                            phone: *phone,
                            house: *house,
                            organization: organization.clone(),
                            permissions: vec![
                                sms3rs_shared::account::Permission::View,
                                sms3rs_shared::account::Permission::Post,
                            ],
                            registration_time: chrono::Utc::now(),
                            password_sha: sha256::digest(password as &str),
                            // yes, 5's default token expire time
                            token_expiration_time: 5,
                        }),
                    ) {
                        return (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED);
                    }

                    if !a.save().await {
                        tracing::error!("Error when saving account {}", a.email());
                    }

                    return (
                        serde_json::to_string(&serde_json::json!({ "account_id": a.id() }))
                            .unwrap(),
                        actix_web::http::StatusCode::OK,
                    );
                }
            }
            AccountVerifyVariant::ResetPassword(password) => {
                if {
                    let a = account.read().await;
                    if a.email() == &email {
                        let id = a.id();
                        drop(a);
                        account_manager.refresh(id).await;
                        true
                    } else {
                        false
                    }
                } {
                    let mut a = account.write().await;

                    if let Err(err) = a.verify(
                        descriptor.code,
                        super::AccountVerifyVariant::ResetPassword(password.to_string()),
                    ) {
                        return (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED);
                    }

                    if !a.save().await {
                        tracing::error!("Error when saving account {}", a.email());
                    }

                    return (String::new(), actix_web::http::StatusCode::OK);
                }
            }
        }
    }

    (
        "Target account not found".to_string(),
        actix_web::http::StatusCode::NOT_FOUND,
    )
}

/// Login to a verified account.
///
/// Url: `/api/account/login/?email={email}`
///
/// Request body: See [`AccountLoginDescriptor`]. (json)
///
/// Response: `200` with `{ "account_id": _, "token": _ }` in json.
#[actix_web::post("/api/account/login/{email}")]
pub async fn login_account(
    actix_web::web::Query(EmailTarget { email }): actix_web::web::Query<EmailTarget>,
    descriptor: actix_web::web::Json<AccountLoginDescriptor>,
) -> impl actix_web::Responder {
    let account_manager = &super::INSTANCE;

    for account in account_manager.inner().read().await.iter() {
        if account.read().await.email() == &email {
            let mut aw = account.write().await;
            let token = aw.login(&descriptor.password);

            if !aw.save().await {
                tracing::error!("Error when saving account {}", aw.email());
            }

            match token {
                Ok(t) => {
                    return (
                        serde_json::to_string(&serde_json::json!({
                            "account_id": aw.id(),
                            "token": t,
                        }))
                        .unwrap(),
                        actix_web::http::StatusCode::OK,
                    );
                }
                Err(err) => {
                    return (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED);
                }
            };
        }
    }

    (
        "Target account not found".to_string(),
        actix_web::http::StatusCode::NOT_FOUND,
    )
}

/// Logout an account.
/// POST only.
///
/// Url: `/api/account/logout`
///
/// Request header: See [`crate::RequestPermissionContext`].
#[actix_web::post("/api/account/logout")]
pub async fn logout_account(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
) -> impl actix_web::Responder {
    let account_manager = &super::INSTANCE;
    match account_manager.index().read().await.get(&cxt.account_id) {
        Some(index) => {
            let b = account_manager.inner().read().await;
            let mut aw = b.get(*index).unwrap().write().await;
            match aw.logout(&cxt.token) {
                Ok(_) => {
                    if !aw.save().await {
                        tracing::error!("Error when saving account {}", aw.email());
                    }
                    return (String::new(), actix_web::http::StatusCode::OK);
                }
                Err(err) => return (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
            }
        }
        None => {
            return (
                "Target account not found".to_string(),
                actix_web::http::StatusCode::NOT_FOUND,
            )
        }
    }
}

/// Log out and remove an verified account.
///
/// Url: `/api/account/signout`
///
/// Request header: See [`crate::RequestPermissionContext`].
///
/// Request body: See [`AccountSignOutDescriptor`]. (json)
#[actix_web::post("/api/account/signout")]
pub async fn sign_out_account(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
    descriptor: actix_web::web::Json<AccountSignOutDescriptor>,
) -> impl actix_web::Responder {
    let account_manager = &super::INSTANCE;
    if match account_manager
        .inner()
        .read()
        .await
        .get(
            match account_manager.index().read().await.get(&cxt.account_id) {
                Some(e) => *e,
                _ => {
                    return (
                        "Target account not found".to_string(),
                        actix_web::http::StatusCode::NOT_FOUND,
                    )
                }
            },
        )
        .unwrap()
        .read()
        .await
        .deref()
    {
        crate::account::Account::Unverified(_) => {
            return (
                "Account unverified".to_string(),
                actix_web::http::StatusCode::FORBIDDEN,
            )
        }
        crate::account::Account::Verified {
            attributes, tokens, ..
        } => {
            sha256::digest(descriptor.password.as_str()) == attributes.password_sha
                && tokens.token_usable(&cxt.token)
        }
    } {
        account_manager.remove(cxt.account_id).await;
        (String::new(), actix_web::http::StatusCode::OK)
    } else {
        (
            "Password incorrect".to_string(),
            actix_web::http::StatusCode::UNAUTHORIZED,
        )
    }
}

/// Get a user's account details.
///
/// Url: `/api/account/view`
///
/// Request header: See [`crate::RequirePermissionContext`].
///
/// Response body: See [`ViewAccountResult`].
#[actix_web::get("/api/account/view")]
pub async fn view_account(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
) -> impl actix_web::Responder {
    let account_manager = &super::INSTANCE;
    match cxt.valid(vec![]).await {
        Ok(_) => {
            let b = account_manager.inner().read().await;
            let a = b
                .get(
                    *account_manager
                        .index()
                        .read()
                        .await
                        .get(&cxt.account_id)
                        .unwrap(),
                )
                .unwrap()
                .read()
                .await;
            match a.deref() {
                crate::account::Account::Unverified(_) => unreachable!(),
                crate::account::Account::Verified { attributes, .. } => (
                    serde_json::to_string(&ViewAccountResult {
                        id: a.id(),
                        metadata: a.metadata().unwrap(),
                        permissions: a.permissions(),
                        registration_time: attributes.registration_time,
                    })
                    .unwrap(),
                    actix_web::http::StatusCode::OK,
                ),
            }
        }
        Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
    }
}

/// Edit account metadata.
///
/// Url: `/api/account/edit`
///
/// Request header: See [`crate::RequirePermissionContext`].
///
/// Request body: See [`AccountEditDescriptor`].
#[actix_web::post("/api/account/edit")]
pub async fn edit_account(
    cxt: actix_web::web::Header<crate::RequirePermissionContext>,
    descriptor: actix_web::web::Json<AccountEditDescriptor>,
) -> impl actix_web::Responder {
    let account_manager = &super::INSTANCE;
    match cxt.valid(vec![]).await {
        Ok(_) => {
            let b = account_manager.inner().read().await;
            let mut a = b
                .get(
                    *account_manager
                        .index()
                        .read()
                        .await
                        .get(&cxt.account_id)
                        .unwrap(),
                )
                .unwrap()
                .write()
                .await;

            for variant in descriptor.into_inner().variants {
                match apply_edit_variant(variant, a.deref_mut()) {
                    Ok(_) => (),
                    Err(err) => return (err.to_string(), actix_web::http::StatusCode::FORBIDDEN),
                }
            }

            if !a.save().await {
                tracing::error!("Error when saving account {}", a.email());
            }

            (String::new(), actix_web::http::StatusCode::OK)
        }
        Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
    }
}

/// Apply an [`AccountEditVariant`] to an account.
/// Not a request handling method.
fn apply_edit_variant(mt: AccountEditVariant, account: &mut super::Account) -> anyhow::Result<()> {
    match account {
        super::Account::Unverified(_) => return Err(anyhow::anyhow!("Account unverified")),
        super::Account::Verified { attributes, .. } => match mt {
            AccountEditVariant::Name(name) => attributes.name = name,
            AccountEditVariant::SchoolId(id) => attributes.school_id = id,
            AccountEditVariant::Phone(phone) => attributes.phone = phone,
            AccountEditVariant::House(house) => attributes.house = house,
            AccountEditVariant::Organization(org) => attributes.organization = org,
            AccountEditVariant::Password { old, new } => {
                if attributes.password_sha == sha256::digest(old) {
                    attributes.password_sha = sha256::digest(new)
                } else {
                    return Err(anyhow::anyhow!("Old password incorrect"));
                }
            }
            AccountEditVariant::TokenExpireTime(time) => attributes.token_expiration_time = time,
        },
    }
    Ok(())
}

/// Initialize a reset password verification.
///
/// Url: `/api/account/reset-password/?email={email}`
#[actix_web::post("/api/account/reset-password")]
pub async fn reset_password(
    actix_web::web::Query(EmailTarget { email }): actix_web::web::Query<EmailTarget>,
) -> impl actix_web::Responder {
    let account_manager = &super::INSTANCE;
    for account in account_manager.inner().read().await.iter() {
        let ar = account.read().await;
        if ar.email() == &email {
            match ar.deref() {
                crate::account::Account::Unverified(_) => {
                    return (
                        "Account unverified".to_string(),
                        actix_web::http::StatusCode::FORBIDDEN,
                    )
                }
                crate::account::Account::Verified { verify, .. } => {
                    if matches!(verify, crate::account::UserVerifyVariant::None) {
                        drop(ar);
                        let mut aw = account.write().await;
                        let e = match aw.deref_mut() {
                            crate::account::Account::Unverified(_) => unreachable!(),
                            crate::account::Account::Verified { verify, .. } => {
                                *verify =
                                    crate::account::UserVerifyVariant::ForgetPassword({
                                        let cxt = crate::account::verify::Context {
                                            email,
                                            code: {
                                                let mut rng = rand::thread_rng();
                                                rng.gen_range(100000..999999)
                                            },
                                            expire_time: chrono::Utc::now().naive_utc()
                                                + chrono::Duration::minutes(15),
                                        };
                                        match cxt.send_verify().await {
                                            Ok(_) => (),
                                            Err(err) => return (
                                                format!("(smtp error) {err}"),
                                                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                                            ),
                                        }
                                        cxt
                                    });
                                (String::new(), actix_web::http::StatusCode::OK)
                            }
                        };
                        if !aw.save().await {
                            tracing::error!("Error when saving account {}", aw.email());
                        }
                        return e;
                    } else {
                        return (
                            "Target account is during verification period".to_string(),
                            actix_web::http::StatusCode::FORBIDDEN,
                        );
                    }
                }
            };
        }
    }

    (
        "Target account not found".to_string(),
        actix_web::http::StatusCode::NOT_FOUND,
    )
}

/// Manage accounts for admins.
pub mod manage {
    use std::{
        hash::{Hash, Hasher},
        ops::{Deref, DerefMut},
    };

    use sms3rs_shared::account::handle::manage::*;

    /// Let admin creating accounts.
    ///
    /// Url: `/api/account/manage/create`
    ///
    /// Request header: See [`crate::RequirePermissionContext`].
    ///
    /// Request body: See [`MakeAccountDescriptor`]. (json)
    ///
    /// Response body: `200` with `{ "account_id": _ }`.
    #[actix_web::post("/api/account/manage/create")]
    pub async fn make_account(
        cxt: actix_web::web::Header<crate::RequirePermissionContext>,
        descriptor: actix_web::web::Json<MakeAccountDescriptor>,
    ) -> impl actix_web::Responder {
        let account_manager = &crate::account::INSTANCE;
        match cxt
            .valid(vec![sms3rs_shared::account::Permission::ManageAccounts])
            .await
        {
            Ok(able) => {
                if !able {
                    return (
                        "Permission denied".to_string(),
                        actix_web::http::StatusCode::FORBIDDEN,
                    );
                }
                let mut b = account_manager.inner().write().await;
                let a = b
                    .get(
                        *account_manager
                            .index()
                            .read()
                            .await
                            .get(&cxt.account_id)
                            .unwrap(),
                    )
                    .unwrap()
                    .read()
                    .await;

                let account = crate::account::Account::Verified {
                    id: {
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        descriptor.email.hash(&mut hasher);
                        hasher.finish()
                    },
                    attributes: crate::account::UserAttributes {
                        email: descriptor.email.clone(),
                        name: descriptor.name.clone(),
                        school_id: descriptor.school_id,
                        phone: descriptor.phone,
                        house: descriptor.house,
                        organization: descriptor.organization.clone(),
                        permissions: descriptor
                            .permissions
                            .iter()
                            // Prevent permission overflowing
                            .filter(|e| a.has_permission(**e))
                            .copied()
                            .collect(),
                        registration_time: chrono::Utc::now(),
                        password_sha: sha256::digest(descriptor.password.as_str()),
                        token_expiration_time: 5,
                    },
                    tokens: crate::account::verify::Tokens::new(),
                    verify: crate::account::UserVerifyVariant::None,
                };

                drop(a);

                if account_manager
                    .index()
                    .read()
                    .await
                    .contains_key(&account.id())
                {
                    return (
                        "Account already exist".to_string(),
                        actix_web::http::StatusCode::CONFLICT,
                    );
                }

                account_manager
                    .index()
                    .write()
                    .await
                    .insert(account.id(), b.len());

                if !account.save().await {
                    tracing::error!("Error when saving account {}", account.email());
                }

                let id = account.id();
                b.push(tokio::sync::RwLock::new(account));

                (
                    serde_json::to_string(&serde_json::json!({ "account_id": id })).unwrap(),
                    actix_web::http::StatusCode::OK,
                )
            }
            Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
        }
    }

    /// View accounts.
    ///
    /// Url: `/api/account/manage/view`
    ///
    /// Request header: See [`crate::RequirePermissionContext`].
    ///
    /// Request body: See [`ViewAccountDescriptor`].
    ///
    /// Response body: `200` with `{ "results": Vec<ViewAccountResult> }`,
    /// also see [`ViewAccountResult`].
    #[actix_web::post("/api/account/manage/view")]
    pub async fn view_account(
        cxt: actix_web::web::Header<crate::RequirePermissionContext>,
        descriptor: actix_web::web::Json<ViewAccountDescriptor>,
    ) -> impl actix_web::Responder {
        let account_manager = &crate::account::INSTANCE;
        match cxt
            .valid(vec![sms3rs_shared::account::Permission::ViewAccounts])
            .await
        {
            Ok(able) => {
                if !able {
                    return (
                        "Permission denied".to_string(),
                        actix_web::http::StatusCode::FORBIDDEN,
                    );
                }

                let ar = account_manager.inner().read().await;
                let mut vec = Vec::new();
                for aid in &descriptor.accounts {
                    let a = ar
                        .get(match account_manager.index().read().await.get(aid) {
                            Some(e) => *e,
                            None => {
                                vec.push(ViewAccountResult::Err {
                                    id: *aid,
                                    error: "Target account not found".to_string(),
                                });
                                continue;
                            }
                        })
                        .unwrap();
                    let account = a.read().await;
                    vec.push(match account.deref() {
                        crate::account::Account::Unverified(_) => ViewAccountResult::Err {
                            id: *aid,
                            error: "Target account is not verified".to_string(),
                        },
                        crate::account::Account::Verified { attributes, .. } => {
                            let permissions = account.permissions();
                            if !cxt.valid(permissions.clone()).await.unwrap() {
                                ViewAccountResult::Err {
                                    id: account.id(),
                                    error: "Permission denied".to_string(),
                                }
                            } else {
                                ViewAccountResult::Ok(super::ViewAccountResult {
                                    id: *aid,
                                    metadata: account.metadata().unwrap(),
                                    permissions,
                                    registration_time: attributes.registration_time,
                                })
                            }
                        }
                    })
                }

                (
                    serde_json::to_string(&serde_json::json!({ "results": vec })).unwrap(),
                    actix_web::http::StatusCode::OK,
                )
            }
            Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
        }
    }

    /// Modify an account from admin side.
    ///
    /// Url: `/api/account/manage/modify`
    ///
    /// Request header: See [`crate::RequirePermissionContext`].
    ///
    /// Request body: See [`ModifyAccountDescriptor`].
    #[actix_web::post("/api/account/manage/modify")]
    pub async fn modify_account(
        cxt: actix_web::web::Header<crate::RequirePermissionContext>,
        descriptor: actix_web::web::Json<ModifyAccountDescriptor>,
    ) -> impl actix_web::Responder {
        let account_manager = &crate::account::INSTANCE;
        match cxt
            .valid(vec![sms3rs_shared::account::Permission::ManageAccounts])
            .await
        {
            Ok(able) => {
                if !able {
                    return (
                        "Permission denied".to_string(),
                        actix_web::http::StatusCode::FORBIDDEN,
                    );
                }

                let ar = account_manager.inner().read().await;
                let mut a = ar
                    .get(
                        match account_manager
                            .index()
                            .read()
                            .await
                            .get(&descriptor.account_id)
                        {
                            Some(e) => *e,
                            None => {
                                return (
                                    "Target account not found".to_string(),
                                    actix_web::http::StatusCode::NOT_FOUND,
                                )
                            }
                        },
                    )
                    .unwrap()
                    .write()
                    .await;

                if !cxt.valid(a.permissions()).await.unwrap_or_default() {
                    return (
                        "Permission denied".to_string(),
                        actix_web::http::StatusCode::FORBIDDEN,
                    );
                }

                for variant in descriptor.into_inner().variants {
                    match apply_account_modify_variant(variant, a.deref_mut(), &cxt).await {
                        Ok(_) => continue,
                        Err(err) => {
                            return (err.to_string(), actix_web::http::StatusCode::FORBIDDEN);
                        }
                    }
                }

                if !a.save().await {
                    tracing::error!("Error when saving account {}", a.email());
                }

                (String::new(), actix_web::http::StatusCode::OK)
            }
            Err(err) => (err.to_string(), actix_web::http::StatusCode::UNAUTHORIZED),
        }
    }

    /// Apply an [`AccountModifyVariant`] to an account.
    /// Not a request handling method.
    async fn apply_account_modify_variant(
        mt: AccountModifyVariant,
        account: &mut crate::account::Account,
        context: &crate::RequirePermissionContext,
    ) -> anyhow::Result<()> {
        match account {
            crate::account::Account::Unverified(_) => {
                return Err(anyhow::anyhow!("Account unverified"))
            }
            crate::account::Account::Verified { attributes, .. } => match mt {
                AccountModifyVariant::Name(name) => attributes.name = name,
                AccountModifyVariant::SchoolId(id) => attributes.school_id = id,
                AccountModifyVariant::Phone(phone) => attributes.phone = phone,
                AccountModifyVariant::House(house) => attributes.house = house,
                AccountModifyVariant::Organization(org) => attributes.organization = org,
                AccountModifyVariant::Email(email) => attributes.email = email,
                AccountModifyVariant::Permission(permissions) => {
                    let am = crate::account::INSTANCE.inner().read().await;
                    let a = am
                        .get(
                            *crate::account::INSTANCE
                                .index()
                                .read()
                                .await
                                .get(&context.account_id)
                                .unwrap(),
                        )
                        .unwrap()
                        .read()
                        .await;
                    attributes.permissions = a
                        .permissions()
                        .into_iter()
                        .filter(|e| permissions.contains(e))
                        .collect();
                }
            },
        }
        Ok(())
    }
}
