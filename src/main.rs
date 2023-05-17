mod account;
pub mod config;
mod post;

use account::{AccountManagerError, Permissions};
use std::ops::Deref;
use tide::Request;

#[async_std::main]
async fn main() -> tide::Result<()> {
    let mut app = tide::new();
    tide::log::with_level(tide::log::LevelFilter::Debug);
    account::INSTANCE.refresh_all().await;

    // Basic account controlling
    app.at("/api/account/create")
        .post(account::handle::create_account);
    app.at("/api/account/verify")
        .post(account::handle::verify_account);
    app.at("/api/account/login")
        .post(account::handle::login_account);
    app.at("/api/account/logout")
        .post(account::handle::logout_account);
    app.at("/api/account/signout")
        .post(account::handle::sign_out_account);
    app.at("/api/account/view")
        .post(account::handle::view_account);
    app.at("/api/account/edit")
        .post(account::handle::edit_account);
    app.at("/api/account/resetpassword")
        .post(account::handle::reset_password);

    // Account managing
    app.at("/api/account/manage/create")
        .post(account::handle::manage::make_account);
    app.at("/api/account/manage/view")
        .post(account::handle::manage::view_account);
    app.at("/api/account/manage/modify")
        .post(account::handle::manage::modify_account);

    app.listen("127.0.0.1:8080").await?;
    Ok(())
}

/// A context for checking the validation of action an account performs with permission requirements.
pub struct RequirePermissionContext {
    /// The access token of this account.
    pub token: String,
    /// The only id of this account.
    pub user_id: u64,
}

impl RequirePermissionContext {
    /// Indicates whether this context's token and permissions is valid.
    pub async fn valid(&self, permissions: Permissions) -> Result<bool, AccountManagerError> {
        let account_manager = account::INSTANCE.deref();
        match account_manager
            .index()
            .read()
            .await
            .get(&self.user_id)
            .map(|e| *e)
        {
            Some(index) => {
                account_manager.refresh(self.user_id).await;
                let b = account_manager.inner().read().await;
                let account = b.get(index).unwrap().read().await;
                Ok(match account.deref() {
                    account::Account::Unverified(_) => {
                        return Err(AccountManagerError::Account(
                            self.user_id,
                            account::AccountError::UserUnverifiedError,
                        ))
                    }
                    account::Account::Verified { tokens, .. } => tokens.token_usable(&self.token),
                } && permissions.iter().all(|p| account.has_permission(*p)))
            }
            None => Err(AccountManagerError::AccountNotFound(self.user_id)),
        }
    }

    pub fn from_header(request: &Request<()>) -> Option<Self> {
        Some(Self {
            token: match request.header("Token") {
                Some(e) => e.as_str().to_string(),
                None => return None,
            },
            user_id: match request.header("AccountId") {
                Some(e) => match e.as_str().parse() {
                    Ok(n) => n,
                    Err(_) => return None,
                },
                None => return None,
            },
        })
    }
}
