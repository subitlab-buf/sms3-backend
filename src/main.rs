use std::sync::Arc;

use dmds::{IoHandle, World};
use lettre::AsyncSmtpTransport;
use sms3_backend::{
    account::{department::Department, Account},
    config::Config,
    Error,
};

fn main() {}

#[derive(Debug, Clone)]
pub struct Global<Io: IoHandle> {
    pub smtp_transport: Arc<AsyncSmtpTransport<lettre::Tokio1Executor>>,
    pub worlds: Arc<Worlds<Io>>,
    pub config: Arc<Config>,
}

type AccountWorld<Io> = World<Account, 1, Io>;
type UnverifiedAccountWorld<Io> = World<sms3_backend::account::Unverified, 1, Io>;
type DepartmentWorld<Io> = World<Department, 1, Io>;

#[derive(Debug)]
pub struct Worlds<Io: IoHandle> {
    account: AccountWorld<Io>,
    unverified_account: UnverifiedAccountWorld<Io>,

    department: DepartmentWorld<Io>,
}

mod handle {
    /// Selects an account.
    macro_rules! sa {
        ($w:expr, $id:expr) => {
            $w.select(0, $id).hint($id)
        };
    }

    /// Gets an account from selection.
    macro_rules! ga {
        ($s:expr, $id:expr) => {{
            let mut iter = $s.iter();
            let mut lazy = None;
            while let Some(Ok(l)) = dmds::StreamExt::next(&mut iter).await {
                if l.id() == $id {
                    lazy = Some(l);
                }
            }
            lazy
        }};
    }

    /// Validates an account.
    macro_rules! va {
        ($a:expr, $s:expr => $($p:expr),*$(,)?) => {{
            let lazy = ga!($s, $a.account).ok_or(Error::PermissionDenied)?;
            let a = lazy.get().await?;
            if a.is_token_valid(&$a.token) {
                let _tags = a.tags();
                if !($(_tags.contains_permission(&sms3_backend::account::Tag::Permission($p)) &&)* true) {
                    return Err($crate::Error::PermissionDenied);
                }
            } else {
                return Err($crate::Error::LibAccount(libaccount::Error::InvalidToken));
            }
            lazy
        }};
        ($a:expr, $s:expr) => {
            va!($a, $s =>)
        }
    }

    pub mod account;
}

#[derive(Debug)]
pub struct Auth {
    account: u64,
    token: String,
}

#[async_trait::async_trait]
impl<Io: IoHandle> axum::extract::FromRequestParts<Global<Io>> for Auth {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &Global<Io>,
    ) -> Result<Self, Self::Rejection> {
        const KEY: &str = "Authorization";
        let raw = parts.headers.remove(KEY).ok_or(Error::NotLoggedIn)?;
        let (account, token) = raw
            .to_str()?
            .split_once(':')
            .ok_or(Error::InvalidAuthHeader)?;
        Ok(Self {
            account: account.parse().map_err(|_| Error::InvalidAuthHeader)?,
            token: token.to_owned(),
        })
    }
}
