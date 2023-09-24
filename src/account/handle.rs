use super::Error;
use super::UserAttributes;
use super::UserVerifyVariant;
use crate::account::verify;
use crate::account::Account;
use crate::account::Permission;
use crate::RequirePermissionContext;
use crate::ResError;
use axum::Json;
use chrono::Duration;
use chrono::Utc;
use parking_lot::RwLock;
use rand::Rng;
use serde_json::json;
use sha256::digest;
use std::ops::Deref;
use std::ops::DerefMut;

use sms3_shared::account::handle::*;

/// Create an unverified account.
pub async fn create_account(
    Json(descriptor): Json<AccountCreateDescriptor>,
) -> axum::response::Result<()> {
    if super::INSTANCE
        .inner()
        .read()
        .iter()
        .any(|account| account.read().email() == &descriptor.email)
    {
        return Err(ResError(super::Error::Conflict).into());
    }

    let len = super::INSTANCE.inner().read().len();
    let account = Account::new(descriptor.email).map_err(ResError)?;

    super::INSTANCE.index().insert(account.id(), len);
    account.save();
    super::INSTANCE.inner().write().push(RwLock::new(account));

    Ok(())
}

/// Verify an account.
pub async fn verify_account(
    Json(descriptor): Json<AccountVerifyDescriptor>,
) -> axum::response::Result<Json<serde_json::Value>> {
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
                let res = {
                    let a = account.read();

                    if a.email() == email {
                        let id = a.id();
                        drop(a);
                        super::INSTANCE.refresh(id);
                        true
                    } else {
                        false
                    }
                };
                if res {
                    let mut a = account.write();

                    a.verify(
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
                    )
                    .map_err(ResError)?;

                    a.save();
                    return Ok(Json(json!({ "account_id": id })));
                }
            }

            AccountVerifyVariant::ResetPassword { email, password } => {
                let res = {
                    let a = account.read();
                    if a.email() == email {
                        let id = a.id();
                        drop(a);
                        super::INSTANCE.refresh(id);
                        true
                    } else {
                        false
                    }
                };
                if res {
                    let mut a = account.write();

                    a.verify(
                        descriptor.code,
                        super::AccountVerifyVariant::ResetPassword(password.to_string()),
                    )
                    .map_err(ResError)?;

                    a.save();
                    return Ok(Json(json!({})));
                }
            }
        }
    }

    Err(ResError(super::ManagerError::NotFound(0)).into())
}

/// Login to a verified account.
pub async fn login_account(
    Json(descriptor): Json<AccountLoginDescriptor>,
) -> axum::response::Result<Json<serde_json::Value>> {
    if let Some(account) = super::INSTANCE
        .inner()
        .read()
        .iter()
        .find(|a| a.read().email() == &descriptor.email)
    {
        let mut aw = account.write();
        let token = aw.login(&descriptor.password);

        aw.save();

        token
            .map(|value| {
                Json(json!({
                    "account_id": aw.id(),
                    "token": value,
                }))
            })
            .map_err(|err| ResError(err).into())
    } else {
        Err(ResError(super::ManagerError::NotFound(0)).into())
    }
}

/// Logout from an account.
pub async fn logout_account(ctx: RequirePermissionContext) -> axum::response::Result<()> {
    let account_manager = &super::INSTANCE;

    if let Some(index) = account_manager
        .index()
        .get(&ctx.account_id)
        .map(|e| *e.value())
    {
        let b = account_manager.inner().read();
        let mut aw = b.get(index).unwrap().write();
        aw.logout(&ctx.token).map_err(ResError)?;
        Ok(())
    } else {
        Err(ResError(super::ManagerError::NotFound(ctx.account_id)).into())
    }
}

/// Sign out and remove an verified account.
pub async fn sign_out_account(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<AccountSignOutDescriptor>,
) -> axum::response::Result<()> {
    let passwd_correct = if let Account::Verified {
        attributes, tokens, ..
    } = super::INSTANCE
        .inner()
        .read()
        .get(
            if let Some(e) = super::INSTANCE.index().get(&ctx.account_id) {
                *e.value()
            } else {
                return Err(ResError(super::ManagerError::NotFound(ctx.account_id)).into());
            },
        )
        .unwrap()
        .read()
        .deref()
    {
        digest(descriptor.password) == attributes.password_sha && tokens.token_usable(&ctx.token)
    } else {
        return Err(ResError(super::Error::UserUnverified).into());
    };

    if passwd_correct {
        super::INSTANCE.remove(ctx.account_id);
        Ok(())
    } else {
        Err(ResError(super::Error::PasswordIncorrect).into())
    }
}

/// Get a user's account details.
pub async fn view_account(
    ctx: RequirePermissionContext,
) -> axum::response::Result<Json<ViewAccountResult>> {
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

    if let Account::Verified { attributes, .. } = a.deref() {
        Ok(Json(ViewAccountResult {
            id: a.id(),
            metadata: a.metadata().unwrap(),
            permissions: a.permissions().to_vec(),
            registration_time: attributes.registration_time,
        }))
    } else {
        unreachable!()
    }
}

/// Edit account metadata.
pub async fn edit_account(
    ctx: RequirePermissionContext,
    Json(descriptor): Json<AccountEditDescriptor>,
) -> axum::response::Result<()> {
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
        apply_edit_variant(variant, a.deref_mut()).map_err(ResError)?;
    }

    a.save();

    Ok(())
}

pub fn apply_edit_variant(mt: AccountEditVariant, account: &mut Account) -> Result<(), Error> {
    match account {
        Account::Unverified(_) => return Err(Error::UserUnverified),
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
                    return Err(Error::PasswordIncorrect);
                }
            }
            AccountEditVariant::TokenExpireTime(time) => attributes.token_expiration_time = time,
        },
    }
    Ok(())
}

/// Initialize a reset password verification.
pub async fn reset_password(
    Json(descriptor): Json<ResetPasswordDescriptor>,
) -> axum::response::Result<()> {
    if let Some(account) = super::INSTANCE
        .accounts
        .read()
        .iter()
        .find(|a| a.read().email() == &descriptor.email)
    {
        let ar = account.read();
        if ar.email() == &descriptor.email {
            return if let Account::Verified { verify, .. } = ar.deref() {
                if matches!(verify, UserVerifyVariant::None) {
                    drop(ar);

                    let mut aw = account.write();

                    if let Account::Verified { verify, .. } = aw.deref_mut() {
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
                    } else {
                        unreachable!()
                    };

                    aw.save();
                    Ok(())
                } else {
                    Err(ResError(super::Error::UserUnverified).into())
                }
            } else {
                Err(ResError(super::Error::UserUnverified).into())
            };
        }
    }

    Err(ResError(super::ManagerError::NotFound(0)).into())
}

/// Manage accounts for admins.
pub mod manage {
    use crate::account::verify::Tokens;
    use crate::account::{self, Error, Permission};
    use crate::account::{Account, UserAttributes};
    use crate::{RequirePermissionContext, ResError};
    use axum::Json;
    use chrono::Utc;
    use parking_lot::RwLock;
    use serde_json::json;
    use sha256::digest;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::ops::{Deref, DerefMut};

    use sms3_shared::account::handle::manage::*;

    /// Let admin creating accounts.
    pub async fn make_account(
        ctx: RequirePermissionContext,
        Json(descriptor): Json<MakeAccountDescriptor>,
    ) -> axum::response::Result<Json<serde_json::Value>> {
        ctx.valid(&[Permission::ManageAccounts]).map_err(ResError)?;

        let mut b = crate::account::INSTANCE.inner().write();
        let a = b
            .get(
                *crate::account::INSTANCE
                    .index()
                    .get(&ctx.account_id)
                    .unwrap()
                    .value(),
            )
            .unwrap()
            .read();

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
                    // prevent permission overflowing
                    .filter(|e| a.has_permission(**e))
                    .copied()
                    .collect(),
                registration_time: Utc::now(),
                password_sha: digest(descriptor.password),
                token_expiration_time: 5,
            },

            tokens: Tokens::new(),
            verify: account::UserVerifyVariant::None,
        };

        drop(a);

        if crate::account::INSTANCE.index().contains_key(&account.id()) {
            return Err(ResError(account::Error::Conflict).into());
        }

        crate::account::INSTANCE
            .index()
            .insert(account.id(), b.len());

        account.save();

        let id = account.id();
        b.push(RwLock::new(account));

        Ok(Json(json!({ "account_id": id })))
    }

    /// View an account.
    pub async fn view_account(
        ctx: RequirePermissionContext,
        Json(descriptor): Json<ViewAccountDescriptor>,
    ) -> axum::response::Result<Json<serde_json::Value>> {
        ctx.valid(&[Permission::ViewAccounts]).map_err(ResError)?;

        let ar = crate::account::INSTANCE.inner().read();
        let mut vec = Vec::new();

        for aid in &descriptor.accounts {
            let a = ar
                .get(if let Some(e) = crate::account::INSTANCE.index().get(aid) {
                    *e
                } else {
                    vec.push(ViewAccountResult::Err {
                        id: *aid,
                        error: "target account not found".to_string(),
                    });
                    continue;
                })
                .unwrap();

            let account = a.read();

            vec.push(
                if let Account::Verified { attributes, .. } = account.deref() {
                    let permissions = account.permissions();
                    if ctx.try_valid(permissions).map_err(ResError)? {
                        ViewAccountResult::Ok(super::ViewAccountResult {
                            id: *aid,
                            metadata: account.metadata().unwrap(),
                            permissions: permissions.to_vec(),
                            registration_time: attributes.registration_time,
                        })
                    } else {
                        ViewAccountResult::Err {
                            id: *aid,
                            error: "permission denied".to_string(),
                        }
                    }
                } else {
                    ViewAccountResult::Err {
                        id: *aid,
                        error: "target account is not verified".to_string(),
                    }
                },
            )
        }

        Ok(Json(
            json!({ "results": serde_json::to_value(vec).unwrap_or_default() }),
        ))
    }

    /// Modify an account from admin side.
    pub async fn modify_account(
        ctx: RequirePermissionContext,
        Json(descriptor): Json<AccountModifyDescriptor>,
    ) -> axum::response::Result<()> {
        ctx.valid(&[Permission::ManageAccounts]).map_err(ResError)?;

        let ar = crate::account::INSTANCE.inner().read();
        let mut a = ar
            .get(
                if let Some(e) = crate::account::INSTANCE.index().get(&descriptor.account_id) {
                    *e.value()
                } else {
                    return Err(
                        ResError(account::ManagerError::NotFound(descriptor.account_id)).into(),
                    );
                },
            )
            .unwrap()
            .write();

        ctx.valid(a.permissions()).map_err(ResError)?;
        for variant in descriptor.variants {
            apply_account_modify_variant(variant, a.deref_mut(), &ctx).map_err(ResError)?;
        }

        a.save();
        Ok(())
    }

    fn apply_account_modify_variant(
        mt: AccountModifyVariant,
        account: &mut Account,
        context: &RequirePermissionContext,
    ) -> Result<(), Error> {
        match account {
            Account::Unverified(_) => return Err(Error::UserUnverified),
            Account::Verified { attributes, .. } => match mt {
                AccountModifyVariant::Name(name) => attributes.name = name,
                AccountModifyVariant::SchoolId(id) => attributes.school_id = id,
                AccountModifyVariant::Phone(phone) => attributes.phone = phone,
                AccountModifyVariant::House(house) => attributes.house = house,
                AccountModifyVariant::Organization(org) => attributes.organization = org,
                AccountModifyVariant::Email(email) => attributes.email = email,
                AccountModifyVariant::Permission(permissions) => {
                    let am = crate::account::INSTANCE.inner().read();
                    let a = am
                        .get(
                            *crate::account::INSTANCE
                                .index()
                                .get(&context.account_id)
                                .unwrap()
                                .value(),
                        )
                        .unwrap()
                        .read();
                    attributes.permissions = a
                        .permissions()
                        .iter()
                        .filter(|e| permissions.contains(e))
                        .copied()
                        .collect();
                }
            },
        }

        Ok(())
    }
}
