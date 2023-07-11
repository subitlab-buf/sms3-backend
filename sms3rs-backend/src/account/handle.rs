use super::AccountError;
use super::UserAttributes;
use super::UserVerifyVariant;
use crate::account::verify;
use crate::account::Account;
use crate::account::Permission;
use crate::RequirePermissionContext;
use axum::http::StatusCode;
use axum::Json;
use chrono::Duration;
use chrono::Utc;
use parking_lot::RwLock;
use rand::Rng;
use serde_json::json;
use sha256::digest;
use std::ops::Deref;
use std::ops::DerefMut;
use tracing::error;
use tracing::info;

use sms3rs_shared::account::handle::*;

/// Create an unverified account.
pub async fn create_account(
    Json(descriptor): Json<AccountCreateDescriptor>,
) -> (StatusCode, Json<serde_json::Value>) {
    if super::INSTANCE
        .inner()
        .read()
        .iter()
        .any(|account| account.read().email() == &descriptor.email)
    {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": "account with target email already exist" })),
        );
    }

    let len = super::INSTANCE.inner().read().len();

    let account = match Account::new(descriptor.email).await {
        Ok(value) => value,
        Err(err) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": err.to_string() })),
            )
        }
    };

    info!(
        "Unverified account created: {} (id {})",
        account.email(),
        account.id()
    );

    super::INSTANCE.index().insert(account.id(), len);

    if !account.save() {
        error!("Error while saving account {}", account.email());
    }

    super::INSTANCE.inner().write().push(RwLock::new(account));

    (StatusCode::OK, Json(json!({})))
}

/// Verify an account.
pub async fn verify_account(
    Json(descriptor): Json<AccountVerifyDescriptor>,
) -> (StatusCode, Json<serde_json::Value>) {
    for account in super::INSTANCE.inner().read().iter() {
        match &descriptor.variant {
            AccountVerifyVariant::Activate {
                email,
                name,
                id,
                phone,
                house,
                organization,
                password,
            } => {
                if {
                    let a = account.read();
                    if a.email() == email {
                        let id = a.id();
                        drop(a);
                        super::INSTANCE.refresh(id);
                        true
                    } else {
                        false
                    }
                } {
                    let mut a = account.write();

                    if let Err(err) = a.verify(
                        descriptor.code,
                        super::AccountVerifyVariant::Activate(UserAttributes {
                            email: email.clone(),
                            name: name.clone(),
                            school_id: *id,
                            phone: *phone,
                            house: *house,
                            organization: organization.clone(),
                            permissions: vec![Permission::View, Permission::Post],
                            registration_time: Utc::now(),
                            password_sha: digest(password as &str),
                            token_expiration_time: 5,
                        }),
                    ) {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(json!({ "error": err.to_string() })),
                        );
                    }

                    if !a.save() {
                        error!("Error when saving account {}", a.email());
                    }

                    info!("Account verified: {} (id: {})", a.email(), a.id());
                    return (StatusCode::OK, Json(json!({ "account_id": id })));
                }
            }

            AccountVerifyVariant::ResetPassword { email, password } => {
                if {
                    let a = account.read();
                    if a.email() == email {
                        let id = a.id();
                        drop(a);
                        super::INSTANCE.refresh(id);
                        true
                    } else {
                        false
                    }
                } {
                    let mut a = account.write();

                    if let Err(err) = a.verify(
                        descriptor.code,
                        super::AccountVerifyVariant::ResetPassword(password.to_string()),
                    ) {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(json!({ "error": err.to_string() })),
                        );
                    }

                    if !a.save() {
                        error!("Error when saving account {}", a.email());
                    }

                    info!("Password reseted: {} (id: {})", a.email(), a.id());
                    return (StatusCode::OK, Json(json!({})));
                }
            }
        }
    }

    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "target account not found" })),
    )
}

/// Login to a verified account.
pub async fn login_account(
    Json(descriptor): Json<AccountLoginDescriptor>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Some(account) = super::INSTANCE
        .inner()
        .read()
        .iter()
        .find(|a| a.read().email() == &descriptor.email)
    {
        let mut aw = account.write();
        let token = aw.login(&descriptor.password);

        if !aw.save() {
            error!("Error when saving account {}", aw.email());
        }

        return match token {
            Ok(t) => {
                info!("Account {} (id: {}) logged in", aw.email(), aw.id());
                (
                    StatusCode::OK,
                    Json(json!({
                        "account_id": aw.id(),
                        "token": t,
                    })),
                )
            }

            Err(err) => (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": err.to_string() })),
            ),
        };
    }

    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "target account not found" })),
    )
}

/// Logout from an account.
pub async fn logout_account(
    ctx: RequirePermissionContext,
) -> (StatusCode, Json<serde_json::Value>) {
    let account_manager = &super::INSTANCE;
    match account_manager
        .index()
        .get(&ctx.account_id)
        .map(|e| *e.value())
    {
        Some(index) => {
            let b = account_manager.inner().read();
            let mut aw = b.get(index).unwrap().write();
            match aw.logout(&ctx.token) {
                Ok(_) => {
                    if !aw.save() {
                        error!("Error when saving account {}", aw.email());
                    }

                    info!("Account {} (id: {}) logged out", aw.email(), aw.id());
                    (StatusCode::OK, Json(json!({})))
                }
                Err(err) => (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": err.to_string() })),
                ),
            }
        }

        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "target account not found" })),
        ),
    }
}

/// Sign out and remove an verified account.
pub async fn sign_out_account(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<AccountSignOutDescriptor>,
) -> (StatusCode, Json<serde_json::Value>) {
    if match super::INSTANCE
        .inner()
        .read()
        .get(match super::INSTANCE.index().get(&ctx.account_id) {
            Some(e) => *e.value(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": "target account not found" })),
                );
            }
        })
        .unwrap()
        .read()
        .deref()
    {
        Account::Unverified(_) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "account unverified" })),
            )
        }

        Account::Verified {
            attributes, tokens, ..
        } => {
            digest(descriptor.password) == attributes.password_sha
                && tokens.token_usable(&ctx.token)
        }
    } {
        super::INSTANCE.remove(ctx.account_id);
        info!("Account {} signed out", ctx.account_id);

        (StatusCode::OK, Json(json!({})))
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "password incorrect" })),
        )
    }
}

/// Get a user's account details.
pub async fn view_account(ctx: RequirePermissionContext) -> (StatusCode, Json<ViewAccountResult>) {
    let b = super::INSTANCE.inner().read();
    let a = b
        .get(
            *super::INSTANCE
                .index()
                .get(&ctx.account_id)
                .unwrap()
                .value(),
        )
        .unwrap()
        .read();
    match a.deref() {
        Account::Unverified(_) => unreachable!(),
        Account::Verified { attributes, .. } => (
            StatusCode::OK,
            Json(ViewAccountResult {
                id: a.id(),
                metadata: a.metadata().unwrap(),
                permissions: a.permissions(),
                registration_time: attributes.registration_time,
            }),
        ),
    }
}

/// Edit account metadata.
pub async fn edit_account(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<AccountEditDescriptor>,
) -> (StatusCode, Json<serde_json::Value>) {
    let b = super::INSTANCE.inner().read();

    let mut a = b
        .get(
            *super::INSTANCE
                .index()
                .get(&ctx.account_id)
                .unwrap()
                .value(),
        )
        .unwrap()
        .write();

    for variant in descriptor.variants {
        match apply_edit_variant(variant, a.deref_mut()) {
            Err(err) => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": err.to_string() })),
                )
            }
            _ => (),
        }
    }

    if !a.save() {
        error!("Error when saving account {}", a.email());
    }

    (StatusCode::OK, Json(json!({})))
}

pub fn apply_edit_variant(
    mt: AccountEditVariant,
    account: &mut Account,
) -> Result<(), AccountError> {
    match account {
        Account::Unverified(_) => return Err(AccountError::UserUnverifiedError),
        Account::Verified { attributes, .. } => match mt {
            AccountEditVariant::Name(name) => attributes.name = name,
            AccountEditVariant::SchoolId(id) => attributes.school_id = id,
            AccountEditVariant::Phone(phone) => attributes.phone = phone,
            AccountEditVariant::House(house) => attributes.house = house,
            AccountEditVariant::Organization(org) => attributes.organization = org,
            AccountEditVariant::Password { old, new } => {
                if attributes.password_sha == digest(old) {
                    attributes.password_sha = digest(new)
                } else {
                    return Err(AccountError::PasswordIncorrectError);
                }
            }
            AccountEditVariant::TokenExpireTime(time) => attributes.token_expiration_time = time,
        },
    }
    Ok(())
}

/// Initialize a reset password verification.
#[axum_macros::debug_handler]
pub async fn reset_password(
    Json(descriptor): Json<ResetPasswordDescriptor>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Some(account) = super::INSTANCE
        .accounts
        .read()
        .iter()
        .find(|a| a.read().email() == &descriptor.email)
    {
        let ar = account.read();
        if ar.email() == &descriptor.email {
            return match ar.deref() {
                Account::Unverified(_) => (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": "target account is not verified" })),
                ),
                Account::Verified { verify, .. } => {
                    if matches!(verify, UserVerifyVariant::None) {
                        drop(ar);

                        let mut aw = account.write();

                        let ret = if let Account::Verified { verify, .. } = aw.deref_mut() {
                            *verify = UserVerifyVariant::ForgetPassword({
                                let ctx = verify::Context {
                                    email: descriptor.email,
                                    code: {
                                        let mut rng = rand::thread_rng();
                                        rng.gen_range(100000..999999)
                                    },
                                    expire_time: Utc::now().naive_utc() + Duration::minutes(15),
                                };

                                ctx.send_verify();

                                ctx
                            });

                            (StatusCode::OK, Json(json!({})))
                        } else {
                            unreachable!()
                        };

                        if !aw.save() {
                            error!("Error when saving account {}", aw.email());
                        }

                        ret
                    } else {
                        (
                            StatusCode::CONFLICT,
                            Json(json!({ "error": "target account is under verification" })),
                        )
                    }
                }
            };
        }
    }

    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "target account not found" })),
    )
}

/// Manage accounts for admins.
pub mod manage {
    use crate::account::verify::Tokens;
    use crate::account::{self, AccountError, Permission};
    use crate::account::{Account, UserAttributes};
    use crate::RequirePermissionContext;
    use chrono::Utc;
    use sha256::digest;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::ops::{Deref, DerefMut};

    use sms3rs_shared::account::handle::manage::*;

    /// Let admin creating accounts.
    pub async fn make_account(mut req: Request<()>) -> tide::Result {
        let account_manager = &account::INSTANCE;
        let context = match RequirePermissionContext::from_header(&req) {
            Some(e) => e,
            None => {
                return Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Permission denied",
                    })
                    .into(),
                )
            }
        };
        let descriptor: MakeAccountDescriptor = req.body_json().await?;
        match context.valid(vec![Permission::ManageAccounts]).await {
            Ok(able) => {
                if !able {
                    return Ok::<tide::Response, tide::Error>(
                        json!({
                            "status": "error",
                            "error": "Permission denied",
                        })
                        .into(),
                    );
                }
                let mut b = account_manager.inner().write().await;
                let a = b
                    .get(
                        *account_manager
                            .index()
                            .read()
                            .await
                            .get(&context.account_id)
                            .unwrap(),
                    )
                    .unwrap()
                    .read()
                    .await;
                let account = Account::Verified {
                    id: {
                        let mut hasher = DefaultHasher::new();
                        descriptor.email.hash(&mut hasher);
                        hasher.finish()
                    },
                    attributes: UserAttributes {
                        email: descriptor.email,
                        name: descriptor.name,
                        school_id: descriptor.school_id,
                        phone: descriptor.phone,
                        house: descriptor.house,
                        organization: descriptor.organization,
                        permissions: descriptor
                            .permissions
                            .iter()
                            // Prevent permission overflowing
                            .filter(|e| a.has_permission(**e))
                            .copied()
                            .collect(),
                        registration_time: Utc::now(),
                        registration_ip: req.remote().map(|e| e.to_string()),
                        password_sha: digest(descriptor.password),
                        token_expiration_time: 5,
                    },
                    tokens: Tokens::new(),
                    verify: account::UserVerifyVariant::None,
                };
                drop(a);
                if account_manager
                    .index()
                    .read()
                    .await
                    .contains_key(&account.id())
                {
                    return Ok::<tide::Response, tide::Error>(
                        json!({
                            "status": "error",
                            "error": "Account already exist"
                        })
                        .into(),
                    );
                }
                account_manager
                    .index()
                    .write()
                    .await
                    .insert(account.id(), b.len());
                if !account.save().await {
                    error!("Error when saving account {}", account.email());
                }
                info!("Account {} (id: {}) built", account.email(), account.id());
                let id = account.id();
                b.push(RwLock::new(account));
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "success",
                        "account_id": id,
                    })
                    .into(),
                )
            }
            Err(err) => Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": err.to_string(),
                })
                .into(),
            ),
        }
    }

    /// View an account.
    pub async fn view_account(mut req: Request<()>) -> tide::Result {
        let account_manager = &account::INSTANCE;
        let context = match RequirePermissionContext::from_header(&req) {
            Some(e) => e,
            None => {
                return Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Permission denied",
                    })
                    .into(),
                )
            }
        };
        let descriptor: ViewAccountDescriptor = req.body_json().await?;
        match context.valid(vec![Permission::ViewAccounts]).await {
            Ok(able) => {
                if !able {
                    return Ok::<tide::Response, tide::Error>(
                        json!({
                            "status": "error",
                            "error": "Permission denied",
                        })
                        .into(),
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
                        Account::Unverified(_) => ViewAccountResult::Err {
                            id: *aid,
                            error: "Target account is not verified".to_string(),
                        },
                        Account::Verified { attributes, .. } => {
                            let permissions = account.permissions();
                            if !context.valid(permissions.clone()).await.unwrap() {
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
                                    registration_ip: attributes.registration_ip.clone(),
                                })
                            }
                        }
                    })
                }
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "success",
                        "results": vec,
                    })
                    .into(),
                )
            }
            Err(err) => Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": err.to_string(),
                })
                .into(),
            ),
        }
    }

    /// Modify an account from admin side.
    pub async fn modify_account(mut req: Request<()>) -> tide::Result {
        let account_manager = &account::INSTANCE;
        let context = match RequirePermissionContext::from_header(&req) {
            Some(e) => e,
            None => {
                return Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": "Permission denied",
                    })
                    .into(),
                )
            }
        };
        let descriptor: AccountModifyDescriptor = req.body_json().await?;
        match context.valid(vec![Permission::ManageAccounts]).await {
            Ok(able) => {
                if !able {
                    return Ok::<tide::Response, tide::Error>(
                        json!({
                            "status": "error",
                            "error": "Permission denied",
                        })
                        .into(),
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
                                return Ok::<tide::Response, tide::Error>(
                                    json!({
                                        "status": "error",
                                        "error": "Target account not found",
                                    })
                                    .into(),
                                )
                            }
                        },
                    )
                    .unwrap()
                    .write()
                    .await;
                if !context.valid(a.permissions()).await.unwrap_or_default() {
                    return Ok::<tide::Response, tide::Error>(
                        json!({
                            "status": "error",
                            "error": "Permission denied",
                        })
                        .into(),
                    );
                }
                for variant in descriptor.variants {
                    match apply_account_modify_variant(variant, a.deref_mut(), &context).await {
                        Ok(_) => continue,
                        Err(err) => {
                            return Ok::<tide::Response, tide::Error>(
                                json!({
                                    "status": "error",
                                    "error": err.to_string(),
                                })
                                .into(),
                            );
                        }
                    }
                }
                if !a.save().await {
                    error!("Error when saving account {}", a.email());
                }
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "success",
                    })
                    .into(),
                )
            }
            Err(err) => Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": err.to_string(),
                })
                .into(),
            ),
        }
    }

    async fn apply_account_modify_variant(
        mt: AccountModifyVariant,
        account: &mut Account,
        context: &RequirePermissionContext,
    ) -> Result<(), AccountError> {
        match account {
            Account::Unverified(_) => return Err(AccountError::UserUnverifiedError),
            Account::Verified { attributes, .. } => match mt {
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
