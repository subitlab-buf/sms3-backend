use super::AccountError;
use super::UserAttributes;
use super::UserVerifyVariant;
use crate::account::verify;
use crate::account::Permission;
use crate::account::{Account, AccountManagerError};
use crate::RequirePermissionContext;
use async_std::sync::RwLock;
use chrono::Duration;
use chrono::Utc;
use rand::Rng;
use sha256::digest;
use std::ops::Deref;
use std::ops::DerefMut;
use tide::log::error;
use tide::log::info;
use tide::prelude::*;
use tide::Request;

use sms3rs_shared::account::handle::*;

/// Create an unverified account.
pub async fn create_account(mut req: Request<()>) -> tide::Result {
    let account_manager = &super::INSTANCE;
    let descriptor: AccountCreateDescriptor = req.body_json().await?;
    for account in account_manager.inner().read().await.iter() {
        if account.read().await.email() == &descriptor.email {
            return Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": "User with this email address already exists",
                })
                .into(),
            );
        }
    }
    let len = account_manager.inner().read().await.len();
    account_manager.inner().write().await.push(RwLock::new({
        let account = match Account::new(descriptor.email).await {
            Ok(e) => e,
            Err(err) => {
                return Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": AccountManagerError::Account(0, err).to_string(),
                    })
                    .into(),
                );
            }
        };
        info!(
            "Unverified account created: {} (id {})",
            account.email(),
            account.id()
        );
        account_manager
            .index()
            .write()
            .await
            .insert(account.id(), len);
        if !account.save().await {
            error!("Error while saving account {}", account.email());
        }
        account
    }));
    Ok::<tide::Response, tide::Error>(
        json!({
            "status": "success",
        })
        .into(),
    )
}

/// Verify an account.
pub async fn verify_account(mut req: Request<()>) -> tide::Result {
    let account_manager = &super::INSTANCE;
    let descriptor: AccountVerifyDescriptor = req.body_json().await?;
    for account in account_manager.inner().read().await.iter() {
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
                let res = {
                    let a = account.read().await;
                    if a.email() == email {
                        let id = a.id();
                        drop(a);
                        account_manager.refresh(id).await;
                        true
                    } else {
                        false
                    }
                };
                if res {
                    let mut a = account.write().await;
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
                            registration_ip: req.remote().map(|s| s.to_string()),
                            password_sha: digest(password as &str),
                            token_expiration_time: 5,
                        }),
                    ) {
                        return Ok::<tide::Response, tide::Error>(
                            json!({
                                "status": "error",
                                "error": AccountManagerError::Account(a.id(), err).to_string(),
                            })
                            .into(),
                        );
                    }
                    if !a.save().await {
                        error!("Error when saving account {}", a.email());
                    }
                    info!("Account verified: {} (id: {})", a.email(), a.id());
                    return Ok::<tide::Response, tide::Error>(
                        json!({
                            "status": "success",
                            "account_id": a.id(),
                        })
                        .into(),
                    );
                }
            }
            AccountVerifyVariant::ResetPassword { email, password } => {
                let res = {
                    let a = account.read().await;
                    if a.email() == email {
                        let id = a.id();
                        drop(a);
                        account_manager.refresh(id).await;
                        true
                    } else {
                        false
                    }
                };
                if res {
                    let mut a = account.write().await;
                    if let Err(err) = a.verify(
                        descriptor.code,
                        super::AccountVerifyVariant::ResetPassword(password.to_string()),
                    ) {
                        return Ok::<tide::Response, tide::Error>(
                            json!({
                                "status": "error",
                                "error": AccountManagerError::Account(a.id(), err).to_string(),
                            })
                            .into(),
                        );
                    }
                    if !a.save().await {
                        error!("Error when saving account {}", a.email());
                    }
                    info!("Password reseted: {} (id: {})", a.email(), a.id());
                    return Ok::<tide::Response, tide::Error>(
                        json!({
                            "status": "success",
                        })
                        .into(),
                    );
                }
            }
        }
    }
    Ok::<tide::Response, tide::Error>(
        json!({
            "status": "error",
            "error": "Target account not found",
        })
        .into(),
    )
}

/// Login to a verified account.
pub async fn login_account(mut req: Request<()>) -> tide::Result {
    let account_manager = &super::INSTANCE;
    let descriptor: AccountLoginDescriptor = req.body_json().await?;
    for account in account_manager.inner().read().await.iter() {
        if account.read().await.email() == &descriptor.email {
            let mut aw = account.write().await;
            let token = aw.login(&descriptor.password);
            if !aw.save().await {
                error!("Error when saving account {}", aw.email());
            }
            return Ok::<tide::Response, tide::Error>(match token {
                Ok(t) => {
                    info!("Account {} (id: {}) logged in", aw.email(), aw.id());
                    json!({
                        "status": "success",
                        "account_id": aw.id(),
                        "token": t,
                    })
                }
                .into(),
                Err(err) => json!({
                    "status": "error",
                    "error": err.to_string(),
                })
                .into(),
            });
        }
    }
    Ok::<tide::Response, tide::Error>(
        json!({
            "status": "error",
            "error": "Target account not found",
        })
        .into(),
    )
}

/// Logout from an account.
pub async fn logout_account(req: Request<()>) -> tide::Result {
    let account_manager = &super::INSTANCE;
    let cxt = match RequirePermissionContext::from_header(&req) {
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
    match account_manager.index().read().await.get(&cxt.user_id) {
        Some(index) => {
            let b = account_manager.inner().read().await;
            let mut aw = b.get(*index).unwrap().write().await;
            match aw.logout(&cxt.token) {
                Err(err) => Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "error",
                        "error": err.to_string(),
                    })
                    .into(),
                ),
                Ok(_) => {
                    if !aw.save().await {
                        error!("Error when saving account {}", aw.email());
                    }
                    info!("Account {} (id: {}) logged out", aw.email(), aw.id());
                    Ok::<tide::Response, tide::Error>(
                        json!({
                            "status": "success",
                        })
                        .into(),
                    )
                }
            }
        }
        None => Ok::<tide::Response, tide::Error>(
            json!({
                "status": "error",
                "error": "Target account not found",
            })
            .into(),
        ),
    }
}

/// Sign out and remove an verified account.
pub async fn sign_out_account(mut req: Request<()>) -> tide::Result {
    let account_manager = &super::INSTANCE;
    let cxt = match RequirePermissionContext::from_header(&req) {
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
    let descriptor: AccountSignOutDescriptor = req.body_json().await?;
    if match account_manager
        .inner()
        .read()
        .await
        .get(
            match account_manager.index().read().await.get(&cxt.user_id) {
                Some(e) => *e,
                _ => {
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
        .read()
        .await
        .deref()
    {
        Account::Unverified(_) => {
            return Ok::<tide::Response, tide::Error>(
                json!({
                    "status": "error",
                    "error": "Account unverified"
                })
                .into(),
            )
        }
        Account::Verified {
            attributes, tokens, ..
        } => {
            digest(descriptor.password) == attributes.password_sha
                && tokens.token_usable(&cxt.token)
        }
    } {
        account_manager.remove(cxt.user_id).await;
        info!("Account {} signed out", cxt.user_id);
        Ok::<tide::Response, tide::Error>(
            json!({
                "status": "success",
            })
            .into(),
        )
    } else {
        Ok::<tide::Response, tide::Error>(
            json!({
                "status": "error",
                "error": "Password incorrect"
            })
            .into(),
        )
    }
}

/// Get a user's account details.
pub async fn view_account(req: Request<()>) -> tide::Result {
    let account_manager = &super::INSTANCE;
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
    match context.valid(vec![]).await {
        Ok(_) => {
            let b = account_manager.inner().read().await;
            let a = b
                .get(
                    *account_manager
                        .index()
                        .read()
                        .await
                        .get(&context.user_id)
                        .unwrap(),
                )
                .unwrap()
                .read()
                .await;
            match a.deref() {
                Account::Unverified(_) => unreachable!(),
                Account::Verified { attributes, .. } => {
                    let result = ViewAccountResult {
                        id: a.id(),
                        metadata: a.metadata().unwrap(),
                        permissions: a.permissions(),
                        registration_time: attributes.registration_time,
                        registration_ip: attributes.registration_ip.clone(),
                    };
                    Ok(json!({
                        "status": "success",
                        "result": result,
                    })
                    .into())
                }
            }
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

/// Edit account metadata.
pub async fn edit_account(mut req: Request<()>) -> tide::Result {
    let account_manager = &super::INSTANCE;
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
    let descriptor: AccountEditDescriptor = req.body_json().await?;
    match context.valid(vec![]).await {
        Ok(_) => {
            let b = account_manager.inner().read().await;
            let mut a = b
                .get(
                    *account_manager
                        .index()
                        .read()
                        .await
                        .get(&context.user_id)
                        .unwrap(),
                )
                .unwrap()
                .write()
                .await;
            for variant in descriptor.variants {
                match apply_metadata_type(variant, a.deref_mut()) {
                    Ok(_) => (),
                    Err(err) => {
                        return Ok::<tide::Response, tide::Error>(
                            json!({
                                "status": "error",
                                "error": err.to_string(),
                            })
                            .into(),
                        )
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

pub fn apply_metadata_type(
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
pub async fn reset_password(mut req: Request<()>) -> tide::Result {
    let account_manager = &super::INSTANCE;
    let descriptor: ResetPasswordDescriptor = req.body_json().await?;
    for account in account_manager.inner().read().await.iter() {
        let ar = account.read().await;
        if ar.email() == &descriptor.email {
            return match ar.deref() {
                Account::Unverified(_) => Ok(json!({
                    "status": "error",
                    "error": "Target account is unverified",
                })
                .into()),
                Account::Verified { verify, .. } => {
                    if matches!(verify, UserVerifyVariant::None) {
                        drop(ar);
                        let mut aw = account.write().await;
                        let e = match aw.deref_mut() {
                            Account::Unverified(_) => unreachable!(),
                            Account::Verified { verify, .. } => {
                                *verify = UserVerifyVariant::ForgetPassword({
                                    let cxt = verify::Context {
                                        email: descriptor.email,
                                        code: {
                                            let mut rng = rand::thread_rng();
                                            rng.gen_range(100000..999999)
                                        },
                                        expire_time: match Utc::now()
                                            .naive_utc()
                                            .checked_add_signed(Duration::minutes(15))
                                        {
                                            Some(e) => e,
                                            _ => {
                                                return Ok(json!({
                                                    "status": "error",
                                                    "error": "Date out of range",
                                                })
                                                .into())
                                            }
                                        },
                                    };
                                    match cxt.send_verify().await {
                                        Ok(_) => (),
                                        Err(err) => {
                                            let e = format!(
                                                "Error while sending verification mail: {}",
                                                err
                                            );
                                            return Ok(json!({
                                                "status": "error",
                                                "error": e,
                                            })
                                            .into());
                                        }
                                    }
                                    cxt
                                });
                                Ok(json!({
                                    "status": "success",
                                })
                                .into())
                            }
                        };
                        if !aw.save().await {
                            error!("Error when saving account {}", aw.email());
                        }
                        e
                    } else {
                        Ok(json!({
                            "status": "error",
                            "error": "Target account is during verification period",
                        })
                        .into())
                    }
                }
            };
        }
    }

    Ok(json!({
        "status": "error",
        "error": "Target account not found",
    })
    .into())
}

/// Manage accounts for admins.
pub mod manage {
    use crate::account::verify::Tokens;
    use crate::account::{self, AccountError, Permission};
    use crate::account::{Account, UserAttributes};
    use crate::RequirePermissionContext;
    use async_std::sync::RwLock;
    use chrono::Utc;
    use sha256::digest;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::ops::{Deref, DerefMut};
    use tide::log::{error, info};
    use tide::prelude::*;
    use tide::Request;

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
                            .get(&context.user_id)
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
                b.push(RwLock::new(account));
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
                            if !context.valid(permissions.clone()).await.unwrap() {}
                            ViewAccountResult::Ok(super::ViewAccountResult {
                                id: *aid,
                                metadata: account.metadata().unwrap(),
                                permissions,
                                registration_time: attributes.registration_time,
                                registration_ip: attributes.registration_ip.clone(),
                            })
                        }
                    })
                }
                Ok::<tide::Response, tide::Error>(
                    json!({
                        "status": "success",
                        "result": vec,
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
                    match apply_account_modify_type(variant, a.deref_mut(), &context).await {
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

    async fn apply_account_modify_type(
        mt: AccountModifyType,
        account: &mut Account,
        context: &RequirePermissionContext,
    ) -> Result<(), AccountError> {
        match account {
            Account::Unverified(_) => return Err(AccountError::UserUnverifiedError),
            Account::Verified { attributes, .. } => match mt {
                AccountModifyType::Name(name) => attributes.name = name,
                AccountModifyType::SchoolId(id) => attributes.school_id = id,
                AccountModifyType::Phone(phone) => attributes.phone = phone,
                AccountModifyType::House(house) => attributes.house = house,
                AccountModifyType::Organization(org) => attributes.organization = org,
                AccountModifyType::Email(email) => attributes.email = email,
                AccountModifyType::Permission(permissions) => {
                    if !context.valid(permissions.clone()).await.unwrap_or(false) {
                        return Err(AccountError::PermissionDeniedError);
                    }
                    attributes.permissions = permissions
                }
            },
        }
        Ok(())
    }
}
