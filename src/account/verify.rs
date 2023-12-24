use std::{collections::HashMap, fmt::Display};

use lettre::{transport::smtp, AsyncSmtpTransport};
use rand::Rng;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{config, Error};

use super::Ext;

/// Verify session variant for a verified account.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum VerifyVariant {
    /// Reset password, if the user forgot it.
    ResetPassword,
}

impl Display for VerifyVariant {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyVariant::ResetPassword => write!(f, "reset password"),
        }
    }
}

/// Context used for account verifying.
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyCx {
    /// The captcha.
    captcha: Captcha,
    /// The time of the last captcha-send request.
    #[serde(with = "time::serde::timestamp")]
    last_req: OffsetDateTime,
}

impl VerifyCx {
    /// Creates a new verify context with the current time
    /// and randomly generated captcha.
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            captcha: Default::default(),
            last_req: OffsetDateTime::UNIX_EPOCH,
        }
    }

    /// Re-request a captcha.
    ///
    /// # Errors
    ///
    /// - Errors if the difference between the last request time
    /// and the current time is no more than 10 minuts.
    pub(super) fn update(&mut self) -> Result<Captcha, Error> {
        const LEAST_DURATION: time::Duration = time::Duration::minutes(10);
        let now = OffsetDateTime::now_utc();
        let delta = now - self.last_req;
        if delta >= LEAST_DURATION {
            self.captcha = Captcha::new();
            self.last_req = OffsetDateTime::now_utc();
            Ok(self.captcha)
        } else {
            Err(Error::ReqTooFrequent(LEAST_DURATION - delta))
        }
    }

    /// Requests to send a captcha with given configuration and `transport`.
    ///
    /// # Errors
    ///
    /// - Errors if the difference between the last request time
    /// and the current time is no more than 10 minutes.
    pub(super) async fn send_email<E>(
        &mut self,
        smtp_config: &config::Smtp,
        to: lettre::Address,
        event: impl Display,
        transport: &AsyncSmtpTransport<E>,
    ) -> Result<(), Error>
    where
        E: lettre::Executor,
        AsyncSmtpTransport<E>: lettre::AsyncTransport<Error = smtp::Error>,
    {
        const SENDER: &str = "SubIT";
        let captcha = self.update()?;

        let msg = lettre::message::Message::builder()
            .sender(lettre::message::Mailbox {
                email: smtp_config.address.to_owned(),
                name: Some(SENDER.to_owned()),
            })
            .to(lettre::message::Mailbox {
                name: None,
                email: to,
            })
            .subject("Your SubIT Screen Management System verification code")
            .body(format!(
                "Your verification code for {event} is: \n\n{captcha}",
            ))?;
        let result = lettre::AsyncTransport::send(transport, msg).await;
        if let Err(err) = result {
            tracing::error!("error sending email with smtp: {err}");
            return Err(err.into());
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn captcha(&self) -> Captcha {
        self.captcha
    }
}

impl Default for VerifyCx {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// A captcha.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Captcha(u32);

impl Captcha {
    const DIGITS: usize = 6;

    /// Creates a new captcha randomly.
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        Self(rng.gen_range(0..1000000))
    }
}

impl Default for Captcha {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Display for Captcha {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let num = self.0.to_string();
        for _ in num.len()..Self::DIGITS {
            '0'.fmt(f)?;
        }
        num.fmt(f)
    }
}

impl libaccount::ExtVerify<super::Tag, Ext> for VerifyCx {
    type Args = Captcha;
    type Error = Error;

    fn into_verified_ext(
        self,
        args: &mut libaccount::VerifyDescriptor<super::Tag, Self::Args>,
    ) -> Result<Ext, Self::Error> {
        // Validate the captcha.
        if self.captcha != args.ext_args {
            return Err(Error::CaptchaIncorrect);
        }

        args.tags.retain_user_definable();
        args.tags.initialize_permissions();

        Ok(Ext {
            verifies: HashMap::new(),
        })
    }
}
