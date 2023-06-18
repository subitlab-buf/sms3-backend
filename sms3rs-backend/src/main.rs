use std::ops::Deref;

mod account;
pub(crate) mod config;
mod post;

/// The module for unit testing, will only be availabled in dev env.
#[cfg(test)]
mod tests;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    account::INSTANCE.refresh_all().await;

    actix_web::HttpServer::new(|| {
        actix_web::App::new()
            .service(account::handle::create_account)
            .service(account::handle::verify_account)
            .service(account::handle::login_account)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await;

    // Basic account controlling
    app.at("/api/account/logout")
        .post(account::handle::logout_account);
    app.at("/api/account/signout")
        .post(account::handle::sign_out_account);
    app.at("/api/account/view")
        .get(account::handle::view_account);
    app.at("/api/account/edit")
        .post(account::handle::edit_account);
    app.at("/api/account/reset-password")
        .post(account::handle::reset_password);

    // Account managing
    app.at("/api/account/manage/create")
        .post(account::handle::manage::make_account);
    app.at("/api/account/manage/view")
        .post(account::handle::manage::view_account);
    app.at("/api/account/manage/modify")
        .post(account::handle::manage::modify_account);

    // Posting
    app.at("/api/post/upload-image")
        .post(post::handle::cache_image);
    app.at("/api/post/get-image").post(post::handle::get_image);
    app.at("/api/post/create").post(post::handle::new_post);
    app.at("/api/post/get").post(post::handle::get_posts);
    app.at("/api/post/edit").post(post::handle::edit_post);
    app.at("/api/post/get-info")
        .post(post::handle::get_posts_info);
    app.at("/api/post/approve").post(post::handle::approve_post);

    Ok(())
}

/// A context for checking the validation of action an account performs with permission requirements.
pub struct RequirePermissionContext {
    /// The only id of this account.
    pub account_id: u64,
    /// The access token of this account.
    pub token: String,
}

impl RequirePermissionContext {
    const HEADER_NAME: &str = "Auth";

    /// Indicates whether this context's token and permissions is valid.
    /// The permissions field can be empty.
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
