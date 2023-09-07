mod account;
pub(crate) mod config;
mod post;

/// The module for unit testing, will only be availabled in dev env.
#[cfg(test)]
mod tests;

use account::{AccountManagerError, Permissions};
use axum::{async_trait, http::StatusCode, routing::post};
use std::ops::Deref;

#[tokio::main]
async fn main() {
    account::INSTANCE.refresh_all();

    // use an external function here so this won't be in a proc macros
    // for betting coding experience, also for tests
    let app = router();

    // socket in 127.0.0.1:8080
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

/// Construct a router.
fn router() -> axum::Router {
    axum::Router::new()
        // account
        .route("/api/account/create", post(account::handle::create_account))
        .route("/api/account/verify", post(account::handle::verify_account))
        .route("/api/account/login", post(account::handle::login_account))
        .route("/api/account/logout", post(account::handle::logout_account))
        .route(
            "/api/account/signout",
            post(account::handle::sign_out_account),
        )
        .route("/api/account/view", post(account::handle::view_account))
        .route("/api/account/edit", post(account::handle::edit_account))
        .route(
            "/api/account/reset-password",
            post(account::handle::reset_password),
        )
        // account management
        .route(
            "/api/account/manage/create",
            post(account::handle::manage::make_account),
        )
        .route(
            "/api/account/manage/view",
            post(account::handle::manage::view_account),
        )
        .route(
            "/api/account/manage/modify",
            post(account::handle::manage::modify_account),
        )
        // posting
        .route("/api/post/upload-image", post(post::handle::cache_image))
        .route("/api/post/get-image", post(post::handle::get_image))
        .route("/api/post/create", post(post::handle::new_post))
        .route("/api/post/get", post(post::handle::get_posts))
        .route("/api/post/edit", post(post::handle::edit_post))
        .route("/api/post/get-info", post(post::handle::get_posts_info))
        .route("/api/post/approve", post(post::handle::approve_post))
}

/// A context for checking the validation of action an account
/// performs with permission requirements.
pub struct RequirePermissionContext {
    /// The access token of this account.
    pub token: String,
    /// The only id of this account.
    pub account_id: u64,
}

impl RequirePermissionContext {
    /// Indicates whether this context's token and permissions is valid.
    pub fn valid(&self, permissions: Permissions) -> Result<bool, AccountManagerError> {
        match account::INSTANCE
            .index()
            .get(&self.account_id)
            .map(|e| *e.value())
        {
            Some(index) => {
                account::INSTANCE.refresh(self.account_id);
                let b = account::INSTANCE.inner().read();
                let account = b.get(index).unwrap().read();
                Ok(match account.deref() {
                    account::Account::Unverified(_) => {
                        return Err(AccountManagerError::Account(
                            self.account_id,
                            account::AccountError::UserUnverified,
                        ))
                    }
                    account::Account::Verified { tokens, .. } => tokens.token_usable(&self.token),
                } && permissions.iter().all(|p| account.has_permission(*p)))
            }
            None => Err(AccountManagerError::AccountNotFound(self.account_id)),
        }
    }
}

#[async_trait]
impl<S> axum::extract::FromRequestParts<S> for RequirePermissionContext {
    type Rejection = (StatusCode, axum::Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let this = Self {
            token: if let Some(value) = parts.headers.get("Token") {
                value.to_str().unwrap_or_default().to_string()
            } else {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    axum::Json(
                        serde_json::json!({ "error": "no valid token field found in headers"}),
                    ),
                ));
            },

            account_id: if let Some(value) = parts.headers.get("AccountId") {
                value
                    .to_str()
                    .unwrap_or_default()
                    .to_string()
                    .parse()
                    .unwrap_or_default()
            } else {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    axum::Json(
                        serde_json::json!({ "error": "no valid account id field found in headers"}),
                    ),
                ));
            },
        };

        if !this.valid(vec![]).unwrap_or_default() {
            return Err((
                StatusCode::FORBIDDEN,
                axum::Json(serde_json::json!({ "error": "permission denied" })),
            ));
        }

        Ok(this)
    }
}
