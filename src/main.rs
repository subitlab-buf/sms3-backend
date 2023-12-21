use std::sync::Arc;

use account::VerifyVariant;
use lettre::{transport::smtp, AsyncSmtpTransport};

mod config;

mod account;

fn main() {
    todo!("routing")
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("account error: {0}")]
    LibAccount(libaccount::Error),
    #[error("verify session \"{0}\" not found")]
    VerifySessionNotFound(VerifyVariant),

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
}

#[derive(Debug, Clone)]
pub struct State {
    smtp_transport: Arc<AsyncSmtpTransport<lettre::Tokio1Executor>>,
}
