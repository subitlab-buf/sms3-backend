use std::{collections::HashSet, num::NonZeroU64};

use axum::{extract::State, Json};
use dmds::{IoHandle, StreamExt};
use libaccount::{tag::AsPermission, Phone, VerifyDescriptor};
use serde::{Deserialize, Serialize};
use sms3_backend::{
    account::{department::Department, verify::Captcha, Permission, Tag, TagEntry, Unverified},
    Error,
};

use crate::{Auth, Global};

#[derive(Deserialize)]
pub struct SendCaptchaReq {
    pub email: lettre::Address,
}

pub async fn send_captcha<Io: IoHandle>(
    State(Global {
        smtp_transport,
        worlds,
        config,
    }): State<Global<Io>>,
    Json(SendCaptchaReq { email }): Json<SendCaptchaReq>,
) -> Result<(), Error> {
    let mut unverified = Unverified::new(email.to_string())?;
    let select = sa!(worlds.account, unverified.email_hash());
    if ga!(select, unverified.email_hash()).is_some() {
        return Err(Error::PermissionDenied);
    }

    let select = worlds
        .unverified_account
        .select(0, unverified.email_hash())
        .hint(unverified.email_hash());
    let mut iter = select.iter();
    while let Some(Ok(mut lazy)) = iter.next().await {
        if lazy.id() == unverified.email_hash() {
            if let Ok(val) = lazy.get_mut().await {
                if val.email() == unverified.email() {
                    val.send_captcha(&config.smtp, &smtp_transport).await?;
                    return Ok(());
                }
            }
        }
    }

    unverified
        .send_captcha(&config.smtp, &smtp_transport)
        .await?;
    worlds.unverified_account.insert(unverified).await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct RegisterReq(pub VerifyDescriptor<Tag, Captcha>);

pub async fn register<Io: IoHandle>(
    State(Global { worlds, .. }): State<Global<Io>>,
    Json(RegisterReq(desc)): Json<RegisterReq>,
) -> Result<(), Error> {
    let unverified = Unverified::new(desc.email.to_owned())?;
    worlds
        .account
        .try_insert(
            libaccount::Unverified::from(
                worlds
                    .unverified_account
                    .chunk_buf_of_data_or_load(&unverified)
                    .await
                    .map_err(|_| Error::UnverifiedAccountNotFound)?
                    .remove(unverified.email_hash())
                    .await
                    .ok_or(Error::UnverifiedAccountNotFound)?,
            )
            .verify(desc)?
            .into(),
        )
        .await
        .map_err(|_| Error::PermissionDenied)
}

#[derive(Deserialize)]
pub struct LoginReq {
    pub email: lettre::Address,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginRes {
    pub id: u64,
    pub token: String,
    pub expire_at: Option<i64>,
}

pub async fn login<Io: IoHandle>(
    State(Global { worlds, .. }): State<Global<Io>>,
    Json(LoginReq { email, password }): Json<LoginReq>,
) -> Result<Json<LoginRes>, Error> {
    let unverified = Unverified::new(email.to_string())?;
    let select = sa!(worlds.account, unverified.email_hash());
    let mut lazy =
        ga!(select, unverified.email_hash()).ok_or(Error::UsernameOrPasswordIncorrect)?;
    let (token, exp_time) = lazy.get_mut().await?.login(&password).map_err(|err| {
        if matches!(err, libaccount::Error::PasswordIncorrect) {
            Error::UsernameOrPasswordIncorrect
        } else {
            err.into()
        }
    })?;

    Ok(axum::Json(LoginRes {
        id: lazy.id(),
        token,
        expire_at: exp_time,
    }))
}

#[derive(Deserialize)]
pub struct SendResetPasswordCaptchaReq {
    pub email: lettre::Address,
}

pub async fn send_reset_password_captcha<Io: IoHandle>(
    State(Global {
        smtp_transport,
        worlds,
        config,
    }): State<Global<Io>>,
    Json(SendResetPasswordCaptchaReq { email }): Json<SendResetPasswordCaptchaReq>,
) -> Result<(), Error> {
    let unverified = Unverified::new(email.to_string())?;
    let select = sa!(worlds.account, unverified.email_hash());
    let mut lazy = ga!(select, unverified.email_hash()).ok_or(Error::PermissionDenied)?;
    lazy.get_mut()
        .await?
        .req_reset_password(&config.smtp, &smtp_transport)
        .await
        .map_err(From::from)
}

#[derive(Deserialize)]
pub struct ResetPasswordReq {
    pub email: lettre::Address,
    pub captcha: Captcha,
    pub new_password: String,
}

pub async fn reset_password<Io: IoHandle>(
    State(Global { worlds, .. }): State<Global<Io>>,
    Json(ResetPasswordReq {
        email,
        captcha,
        new_password,
    }): Json<ResetPasswordReq>,
) -> Result<(), Error> {
    let unverified = Unverified::new(email.to_string())?;
    let select = sa!(worlds.account, unverified.email_hash());
    let mut lazy = ga!(select, unverified.email_hash()).ok_or(Error::PermissionDenied)?;
    lazy.get_mut().await?.reset_password(captcha, new_password)
}

#[derive(Serialize)]
pub struct SelfInfoRes {
    pub email: lettre::Address,
    pub name: String,
    pub school_id: String,
    pub phone: Option<Phone>,

    /// Duration, as seconds.
    pub token_expire_duration: Option<NonZeroU64>,

    pub permissions: Vec<Permission>,
    pub departments: Vec<Department>,
}

pub async fn self_info<Io: IoHandle>(
    auth: Auth,
    State(Global { worlds, .. }): State<Global<Io>>,
) -> Result<Json<SelfInfoRes>, Error> {
    let select = sa!(worlds.account, auth.account);
    let lazy = va!(auth, select);
    let account = lazy.get().await?;
    Ok(Json(SelfInfoRes {
        email: account.email().parse()?,
        name: account.name().to_owned(),
        school_id: account.school_id().to_owned(),
        phone: account.phone(),
        token_expire_duration: account.token_expire_time(),
        permissions: account
            .tags()
            .from_entry(&TagEntry::Permission)
            .map_or(vec![], |set| {
                set.into_iter()
                    .filter_map(|t| t.as_permission())
                    .copied()
                    .collect()
            }),
        departments: account
            .tags()
            .from_entry(&TagEntry::Department)
            .map_or(vec![], |set| {
                set.into_iter()
                    .filter_map(|t| {
                        if let Tag::Department(d) = t {
                            Some(d.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            }),
    }))
}

#[derive(Deserialize)]
pub struct ModifyReq {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub school_id: Option<String>,
    #[serde(default)]
    pub phone: Option<Phone>,

    /// Duration, as seconds.
    /// Zero means never expires.
    #[serde(default)]
    pub token_expire_duration: Option<u64>,
    #[serde(default)]
    pub password: Option<ModifyPasswordPart>,

    #[serde(default)]
    pub departments: Option<Vec<Department>>,
}

#[derive(Deserialize)]
pub struct ModifyPasswordPart {
    pub old: String,
    pub new: String,
}

pub async fn modify<Io: IoHandle>(
    auth: Auth,
    State(Global { worlds, .. }): State<Global<Io>>,
    Json(mut req): Json<ModifyReq>,
) -> Result<(), Error> {
    let select = sa!(worlds.account, auth.account);
    let mut lazy = va!(auth, select);
    let account = lazy.get_mut().await?;

    macro_rules! modify {
        ($($i:ident => $m:ident),*$(,)?) => { $(if let Some(v) = req.$i.take() { account.$m(v) })* };
    }
    modify! {
        name => set_name,
        school_id => set_school_id,
        phone => set_phone
    }
    if let Some(dur) = req.token_expire_duration.and_then(NonZeroU64::new) {
        account.set_token_expire_time(Some(dur.get()))
    }
    if let Some(ModifyPasswordPart { old, new }) = req.password.take() {
        if account.password_matches(&old) {
            account.set_password(new)
        } else {
            return Err(Error::UsernameOrPasswordIncorrect);
        }
    }
    if let Some(mut departments) = req.departments.take() {
        account
            .tags_mut()
            .from_entry_mut(&TagEntry::Department)
            .map(|t| t.clear());
        departments.iter_mut().for_each(Department::initialize_id);
        let mut di = departments.iter();
        if let Some(first) = di.next() {
            let mut select = worlds.department.select(0, first.id());
            for department in di {
                select = select.plus(0, department.id())
            }
            let mut iter = select.iter();
            while let Some(Ok(l)) = iter.next().await {
                departments.iter().position(|d| l.id() == d.id()).map(|i| {
                    account
                        .tags_mut()
                        .insert(Tag::Department(departments.swap_remove(i)))
                });
            }
        }
    }

    Ok(())
}

pub async fn logout<Io: IoHandle>(
    auth: Auth,
    State(Global { worlds, .. }): State<Global<Io>>,
) -> Result<(), Error> {
    let select = sa!(worlds.account, auth.account);
    let mut lazy = va!(auth, select);
    lazy.get_mut()
        .await?
        .logout(&auth.token)
        .map_err(From::from)
}

#[derive(Deserialize)]
pub struct SetPermissionsReq {
    pub target_account: u64,
    pub permissions: Vec<Permission>,
}

pub async fn set_permissions<Io: IoHandle>(
    auth: Auth,
    State(Global { worlds, .. }): State<Global<Io>>,
    Json(SetPermissionsReq {
        target_account,
        permissions,
    }): Json<SetPermissionsReq>,
) -> Result<(), Error> {
    let select = sa!(worlds.account, auth.account);
    let lazy = va!(auth, select => Permission::SetPermissions);
    let this = lazy.get().await?;
    let permissions: HashSet<_> = permissions.into_iter().map(From::from).collect();
    let legal_perms = permissions
        .intersection(
            this.tags()
                .from_entry(&TagEntry::Permission)
                .ok_or(Error::PermissionDenied)?,
        )
        .filter_map(Tag::as_permission)
        .copied();

    let select_t = sa!(worlds.account, target_account);
    let mut lazy_t = ga!(select_t, target_account).ok_or(Error::TargetAccountNotFound)?;
    let target = lazy_t.get_mut().await?;
    if this
        .tags()
        .from_entry(&TagEntry::Permission)
        .map_or(false, |p| {
            target
                .tags()
                .from_entry(&TagEntry::Permission)
                .map_or(true, |pt| pt.is_subset(p))
        })
    {
        target.tags_mut().initialize_permissions();
        *target
            .tags_mut()
            .from_entry_mut(&TagEntry::Permission)
            .unwrap() = legal_perms.into_iter().map(From::from).collect();
    }

    Ok(())
}
