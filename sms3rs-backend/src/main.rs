use std::ops::Deref;

mod account;
mod config;
mod post;

/// The module for unit testing which
/// only available in dev env.
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    account::INSTANCE.refresh_all().await;

    actix_web::HttpServer::new(|| {
        actix_web::App::new()
            // basic account controlling
            .service(account::handle::create_account)
            .service(account::handle::verify_account)
            .service(account::handle::login_account)
            .service(account::handle::sign_out_account)
            .service(account::handle::view_account)
            .service(account::handle::edit_account)
            .service(account::handle::reset_password)
            // account management
            .service(account::handle::manage::make_account)
            .service(account::handle::manage::view_account)
            .service(account::handle::manage::modify_account)
            // posting
            .service(post::handle::cache_image)
            .service(post::handle::get_image)
            .service(post::handle::create_post)
            .service(post::handle::get_posts)
            .service(post::handle::edit_post)
            .service(post::handle::get_posts_info)
            .service(post::handle::approve_post)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

/// Context for checking the validation of action an
/// account performs with permission requirements.
///
/// # Use as a header
///
/// Header name: See [`Self::HEADER_NAME`].
/// Header value: `{account_id$token}`.
pub struct RequirePermissionContext {
    /// The only id of this account.
    pub account_id: u64,
    /// The access token of this account.
    pub token: String,
}

impl RequirePermissionContext {
    /// Header name of this context should in.
    const HEADER_NAME: &str = "Auth";

    /// Whether this context's token and permissions is valid.
    /// The permissions field can be empty when no permission is required.
    pub async fn valid(
        &self,
        permissions: sms3rs_shared::account::Permissions,
    ) -> anyhow::Result<bool> {
        let account_manager = account::INSTANCE.deref();
        match account_manager
            .index()
            .read()
            .await
            .get(&self.account_id)
            .copied()
        {
            Some(index) => {
                account_manager.refresh(self.account_id).await;
                let b = account_manager.inner().read().await;
                let account = b.get(index).unwrap().read().await;
                Ok(match account.deref() {
                    account::Account::Unverified(_) => {
                        return Err(anyhow::anyhow!("Account unverified"))
                    }
                    account::Account::Verified { tokens, .. } => tokens.token_usable(&self.token),
                } && permissions.iter().all(|p| account.has_permission(*p)))
            }
            None => Err(anyhow::anyhow!("Target account found")),
        }
    }
}

impl actix_web::http::header::TryIntoHeaderValue for RequirePermissionContext {
    type Error = actix_web::http::header::InvalidHeaderValue;

    fn try_into_value(self) -> Result<actix_web::http::header::HeaderValue, Self::Error> {
        actix_web::http::header::HeaderValue::from_str(&format!(
            "{}${}",
            self.account_id, self.token
        ))
    }
}

impl actix_web::http::header::Header for RequirePermissionContext {
    fn name() -> actix_web::http::header::HeaderName {
        actix_web::http::header::HeaderName::from_static(Self::HEADER_NAME)
    }

    fn parse<M: actix_web::HttpMessage>(msg: &M) -> Result<Self, actix_web::error::ParseError> {
        let header = msg
            .headers()
            .get(Self::HEADER_NAME)
            .ok_or(actix_web::error::ParseError::Header)?
            .to_str()
            .map_err(|_| actix_web::error::ParseError::Header)?
            .split_once('$')
            .ok_or(actix_web::error::ParseError::Header)?;

        Ok(Self {
            account_id: header
                .0
                .parse()
                .map_err(|_| actix_web::error::ParseError::Header)?,
            token: header.1.to_string(),
        })
    }
}
