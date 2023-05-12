use super::AccountError;
use crate::config;
use chrono::{Days, NaiveDateTime, Utc};
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncStd1Executor, AsyncTransport, Message,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use tide::log::info;

pub(super) static SENDER_INSTANCE: Lazy<VerificationSender> =
    Lazy::new(|| VerificationSender::new());

/// Represent infos of an unverified object.
#[derive(Serialize, Deserialize, Debug)]
pub struct Context {
    /// The email address.
    pub(super) email: lettre::Address,
    /// The pending verification code with 6 digits.
    pub(super) code: u32,
    /// The expire time of this context.
    pub(super) expire_time: NaiveDateTime,
}

impl Context {
    pub async fn send_verify(&self) -> Result<(), AccountError> {
        info!(
            "Sending verification code for {} (code: {})",
            self.email, self.code
        );
        SENDER_INSTANCE
            .send_verification(&self)
            .await
            .map_err(|err| AccountError::MailSendError(err.to_string()))?;
        info!("Verification code for {} sent", self.email);
        Ok(())
    }

    /// Whether this context was expired.
    pub fn is_expired(&self) -> bool {
        self.expire_time <= Utc::now().naive_utc()
    }
}

/// A simple token manager.
#[derive(Serialize, Deserialize, Debug)]
pub struct Tokens {
    inner: Vec<(Option<NaiveDateTime>, u64)>,
}

impl Tokens {
    pub fn new() -> Self {
        Self {
            inner: Vec::with_capacity(16),
        }
    }

    /// Create a new token.
    pub(super) fn new_token(
        &mut self,
        // The user id.
        id: u64,
        expire_time: u16,
    ) -> u64 {
        let now = if expire_time == 0 {
            None
        } else {
            Some(
                Utc::now()
                    .naive_utc()
                    .checked_add_days(Days::new(expire_time as u64))
                    .unwrap_or_default(),
            )
        };
        let token: u64 = {
            let mut hasher = DefaultHasher::new();
            id.hash(&mut hasher);
            now.hash(&mut hasher);
            hasher.finish()
        };
        if self.inner.capacity() == self.inner.len() + 1 {
            self.inner.remove(self.inner.len());
        }
        self.inner.push((now, token));
        token
    }

    /// Remove a target token and return whether the token was be removed successfully.
    pub(super) fn remove(&mut self, token: u64) -> bool {
        let l = self.inner.len();
        self.inner = self
            .inner
            .iter()
            .filter(|e| e.1 != token)
            .map(|e| e.clone())
            .collect();
        l > self.inner.len()
    }

    /// Check if a token is usable.
    pub fn token_usable(&self, token: u64) -> bool {
        self.inner.iter().any(|e| e.1 == token)
    }

    /// Remove expired tokens.
    pub fn refresh(&mut self) {
        self.inner = self
            .inner
            .iter()
            .filter(|e| e.0.map_or(true, |a| a > Utc::now().naive_utc()))
            .map(|e| e.clone())
            .collect();
        self.inner.sort_by(|a, b| b.0.cmp(&a.0));
    }
}

pub struct VerificationSender {
    config: &'static crate::config::MailSmtp,
}

impl VerificationSender {
    pub fn new() -> Self {
        Self {
            config: &config::INSTANCE.mail_smtp,
        }
    }

    fn mailer(&self) -> AsyncSmtpTransport<AsyncStd1Executor> {
        AsyncSmtpTransport::<AsyncStd1Executor>::relay(&self.config.server)
            .unwrap()
            .port(self.config.port)
            .credentials(Credentials::new(
                self.config.username.clone(),
                self.config.password.clone(),
            ))
            .build()
    }

    pub async fn send_verification(
        &self,
        cxt: &Context,
    ) -> Result<(), lettre::transport::smtp::Error> {
        let mailer = self.mailer();
        mailer
            .send(
                Message::builder()
                    .from(Mailbox::new(
                        Some("SubIT".to_string()),
                        self.config.address.clone(),
                    ))
                    .to(Mailbox::new(None, cxt.email.clone()))
                    .subject("Your verification code")
                    .header(ContentType::TEXT_PLAIN)
                    .body(format!("Your verification code is {}", cxt.code))
                    .unwrap(),
            )
            .await
            .map(|_| ())?;
        Ok(())
    }
}
