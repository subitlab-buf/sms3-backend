use super::AccountError;
use chrono::{Days, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sha256::digest;

#[cfg(not(test))]
use once_cell::sync::Lazy;

use tracing::info;

#[cfg(not(test))]
pub(super) static SENDER_INSTANCE: Lazy<VerificationSender> = Lazy::new(VerificationSender::new);

#[cfg(test)]
pub static VERIFICATION_CODE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Represent infos of an unverified object.
#[derive(Serialize, Deserialize, Debug)]
pub struct Context {
    /// The email address.
    pub email: lettre::Address,
    /// The pending verification code with 6 digits.
    pub code: u32,
    /// The expire time of this context.
    pub expire_time: NaiveDateTime,
}

impl Context {
    pub async fn send_verify(&self) -> Result<(), AccountError> {
        info!(
            "Sending verification code for {} (code: {})",
            self.email, self.code
        );

        #[cfg(not(test))]
        {
            SENDER_INSTANCE
                .send_verification(self)
                .await
                .map_err(|err| AccountError::MailSendError(err.to_string()))?;
        }

        #[cfg(test)]
        {
            VERIFICATION_CODE.store(self.code, std::sync::atomic::Ordering::Relaxed);
        }

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
    inner: Vec<(Option<NaiveDateTime>, String)>,
}

impl Tokens {
    pub fn new() -> Self {
        Self {
            inner: Vec::with_capacity(16),
        }
    }

    /// Create a new token.
    #[must_use]
    pub fn new_token(
        &mut self,
        // The user id.
        id: u64,
        expire_time: u16,
    ) -> String {
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
        let token = digest(format!("{}-{:?}", id, now));
        if self.inner.capacity() == self.inner.len() + 1 {
            self.inner.remove(self.inner.len());
        }
        self.inner.push((now, token.clone()));
        token
    }

    /// Remove a target token and return whether the token was be removed successfully.
    pub(super) fn remove(&mut self, token: &str) -> bool {
        let l = self.inner.len();
        self.inner.retain(|e| e.1 != token);
        l > self.inner.len()
    }

    /// Check if a token is usable.
    pub fn token_usable(&self, token: &str) -> bool {
        self.inner.iter().any(|e| e.1 == token)
    }

    /// Remove expired tokens.
    pub fn refresh(&mut self) {
        self.inner
            .retain(|e| e.0.map_or(true, |a| a > Utc::now().naive_utc()));
        self.inner.sort_by(|a, b| b.0.cmp(&a.0));
    }
}

#[cfg(not(test))]
pub struct VerificationSender {
    config: &'static crate::config::MailSmtp,
}

#[cfg(not(test))]
impl VerificationSender {
    pub fn new() -> Self {
        Self {
            config: &crate::config::INSTANCE.mail_smtp,
        }
    }

    fn mailer(&self) -> lettre::AsyncSmtpTransport<lettre::Tokio1Executor> {
        use lettre::{
            transport::smtp::authentication::Credentials, AsyncSmtpTransport, Tokio1Executor,
        };

        AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.server)
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
        use lettre::{
            message::{header::ContentType, Mailbox},
            AsyncTransport, Message,
        };

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
