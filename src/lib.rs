use account::verify::VerifyVariant;
use axum::{http::StatusCode, response::IntoResponse};
use lettre::transport::smtp;
use serde::Serialize;

pub mod config;

pub mod account;
pub mod post;

pub mod resource;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("account error: {0}")]
    LibAccount(libaccount::Error),
    #[error("verify session \"{0}\" not found")]
    VerifySessionNotFound(VerifyVariant),
    #[error("permission denied")]
    PermissionDenied,
    #[error("unverified account not found")]
    UnverifiedAccountNotFound,
    #[error("username or password incorrect")]
    UsernameOrPasswordIncorrect,
    #[error("target operation account not found")]
    TargetAccountNotFound,

    #[error("captcha incorrect")]
    CaptchaIncorrect,
    #[error("request too frequent, try after {0}")]
    ReqTooFrequent(time::Duration),

    #[error("address error: {0}")]
    EmailAddress(lettre::address::AddressError),
    #[error("email message error: {0}")]
    Lettre(lettre::error::Error),
    #[error("failed to send email")]
    Smtp(smtp::Error),

    #[error("resource upload session {0} not found")]
    ResourceUploadSessionNotFound(u64),

    #[error("not logged in")]
    NotLoggedIn,
    #[error("non-ascii header value: {0}")]
    HeaderNonAscii(axum::http::header::ToStrError),
    #[error("auth header is not in {{account}}:{{token}} syntax")]
    InvalidAuthHeader,

    #[error("database errored")]
    Database(dmds::Error),

    #[error("unknown")]
    Unknown,
}

impl Error {
    pub fn to_status_code(&self) -> StatusCode {
        match self {
            Error::VerifySessionNotFound(_)
            | Error::ResourceUploadSessionNotFound(_)
            | Error::TargetAccountNotFound
            | Error::UnverifiedAccountNotFound => StatusCode::NOT_FOUND,
            Error::ReqTooFrequent(_) => StatusCode::TOO_MANY_REQUESTS,
            Error::EmailAddress(_) => StatusCode::BAD_REQUEST,
            Error::Lettre(_) | Error::Smtp(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::NotLoggedIn => StatusCode::UNAUTHORIZED,
            Error::HeaderNonAscii(_) | Error::InvalidAuthHeader => StatusCode::BAD_REQUEST,
            Error::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Unknown => StatusCode::IM_A_TEAPOT,
            _ => StatusCode::FORBIDDEN,
        }
    }
}

impl IntoResponse for Error {
    #[inline]
    fn into_response(self) -> axum::response::Response {
        #[derive(Serialize)]
        struct ErrorInfo {
            error: String,
        }
        (
            self.to_status_code(),
            axum::Json(ErrorInfo {
                error: self.to_string(),
            }),
        )
            .into_response()
    }
}

/// Implements `From<T>` for [`Error`].
macro_rules! impl_from {
    ($($t:ty => $v:ident),* $(,)?) => {
        $(
            impl From<$t> for $crate::Error {
                #[inline]
                fn from(err: $t) -> Self {
                    Self::$v(err)
                }
            }
        )*
    };
}

impl_from! {
    libaccount::Error => LibAccount,
    lettre::address::AddressError => EmailAddress,
    lettre::error::Error => Lettre,
    smtp::Error => Smtp,
    axum::http::header::ToStrError => HeaderNonAscii,
    dmds::Error => Database,
}
