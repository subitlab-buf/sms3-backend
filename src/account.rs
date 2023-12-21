use std::{
    collections::HashMap,
    fmt::Display,
    ops::{Deref, DerefMut},
};

use lettre::{transport::smtp, AsyncSmtpTransport};
use rand::Rng;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{config, Error};

/// A permission group of an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Permission {
    /// Contains all permissions.
    Op,
    /// Post postings.
    ///
    /// # Containing permissions
    ///
    /// - [`Self::GetPubPost`]
    Post,
    /// Get public posts.
    GetPubPost,
}

impl libaccount::Permission for Permission {
    #[inline]
    fn default_set() -> libaccount::Permissions<Self> {
        libaccount::Permissions::empty()
    }

    #[inline]
    fn contains(&self, permission: &Self) -> bool {
        matches!(
            (self, permission),
            (Permission::Op, _) | (Permission::Post, Permission::GetPubPost)
        )
    }
}

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

/// The external data of a verified account.
/// Containing verify sessions.
#[derive(Serialize, Deserialize, Debug)]
pub struct Ext {
    verifies: HashMap<VerifyVariant, VerifyCx>,
}

/// A verified account.
///
/// # Verify Sessions
///
/// Sessions that requires email verifying, like reseting password,
/// are verify sessions. (See [`VerifyVariant`]).
/// Verify sessions are stored in external data as [`Ext`].
///
/// Currently, the only verify session is resetting password.
#[derive(Debug)]
pub struct Account {
    inner: libaccount::Account<Permission, Ext>,
}

impl Account {
    /// Requests to reset password and sends an email to user.
    ///
    /// # Errors
    ///
    /// - Errors if the difference between the last request time
    /// and the current time is no more than 10 minutes.
    /// - Errors if the email send failed.
    #[inline]
    pub async fn req_reset_password<E>(
        &mut self,
        config: &config::Smtp,
        transport: &AsyncSmtpTransport<E>,
    ) -> Result<(), Error>
    where
        E: lettre::Executor,
        AsyncSmtpTransport<E>: lettre::AsyncTransport<Error = smtp::Error>,
    {
        self.req_verify(VerifyVariant::ResetPassword, config, transport)
            .await
    }

    /// Resets the password with given new password.
    ///
    /// # Errors
    ///
    /// - Errors if the captcha is incorrect.
    #[inline]
    pub fn reset_password<T>(&mut self, captcha: Captcha, new_password: T) -> Result<(), Error>
    where
        T: AsRef<str>,
    {
        self.do_verify(VerifyVariant::ResetPassword, captcha)?;
        self.inner.set_password(new_password);
        Ok(())
    }

    /// Requests a verify session and sends an email to user.
    ///
    /// # Errors
    ///
    /// - Errors if the difference between the last request time
    /// and the current time is no more than 10 minutes.
    /// - Errors if the email send failed.
    async fn req_verify<E>(
        &mut self,
        variant: VerifyVariant,
        config: &config::Smtp,
        transport: &AsyncSmtpTransport<E>,
    ) -> Result<(), Error>
    where
        E: lettre::Executor,
        AsyncSmtpTransport<E>: lettre::AsyncTransport<Error = smtp::Error>,
    {
        let to = self.inner.email().parse()?;
        let ext = self.inner.ext_mut();
        if let Some(cx) = ext.verifies.get_mut(&variant) {
            cx.update()?;
        } else {
            ext.verifies.insert(variant, VerifyCx::new());
        }
        ext.verifies
            .get_mut(&variant)
            .unwrap()
            .send_email(config, to, variant, transport)
            .await
    }

    /// Validates the verify session captcha and removes the session entry
    /// if the captcha is correct, or throw an error.
    fn do_verify(&mut self, variant: VerifyVariant, captcha: Captcha) -> Result<(), Error> {
        if self
            .inner
            .ext()
            .verifies
            .get(&variant)
            .ok_or(Error::VerifySessionNotFound(variant))?
            .captcha
            == captcha
        {
            self.inner.ext_mut().verifies.remove(&variant);
            Ok(())
        } else {
            Err(Error::CaptchaIncorrect)
        }
    }
}

impl From<libaccount::Account<Permission, Ext>> for Account {
    #[inline]
    fn from(inner: libaccount::Account<Permission, Ext>) -> Self {
        Self { inner }
    }
}

impl dmds::Data for Account {
    const DIMS: usize = 1;
    const VERSION: u32 = 1;

    #[inline]
    fn dim(&self, dim: usize) -> u64 {
        match dim {
            0 => self.id(),
            _ => unreachable!(),
        }
    }

    fn decode<B: bytes::Buf>(version: u32, dims: &[u64], buf: B) -> std::io::Result<Self> {
        match version {
            1 => {
                let mut inner: libaccount::Account<Permission, Ext> =
                    bincode::deserialize_from(buf.reader())
                        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
                unsafe { inner.initialize_id(dims[0]) };
                Ok(Self { inner })
            }
            _ => unreachable!("unsupported data version {version}"),
        }
    }

    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: B) -> std::io::Result<()> {
        bincode::serialize_into(buf.writer(), &self.inner)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
    }
}

impl Deref for Account {
    type Target = libaccount::Account<Permission, Ext>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Account {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// An unverified account.
#[derive(Debug)]
pub struct Unverified {
    inner: libaccount::Unverified<VerifyCx>,
}

impl Unverified {
    /// Creates a new unverified account.
    ///
    /// # Errors
    ///
    /// - Errors if email is not ended with `@pkuschool.edu.cn`
    /// or `@i.pkuschool.edu.cn`.
    #[inline]
    pub fn new(email: String) -> Result<Self, Error> {
        Ok(Self {
            inner: libaccount::Unverified::new(email, VerifyCx::new())?,
        })
    }

    /// Requests to send a captcha with given configuration and `transport`.
    ///
    /// # Errors
    ///
    /// - Errors if the difference between the last request time
    /// and the current time is no more than 10 minutes.
    /// - Errors if the email send failed.
    pub async fn send_captcha<E>(
        &mut self,
        config: &config::Smtp,
        transport: &AsyncSmtpTransport<E>,
    ) -> Result<(), Error>
    where
        E: lettre::Executor,
        AsyncSmtpTransport<E>: lettre::AsyncTransport<Error = smtp::Error>,
    {
        let to = self.inner.email().parse()?;
        self.inner
            .ext_mut()
            .send_email(config, to, "account activation", transport)
            .await
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
    fn new() -> Self {
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
    fn update(&mut self) -> Result<Captcha, Error> {
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
    async fn send_email<E>(
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
}

impl Default for VerifyCx {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl dmds::Data for Unverified {
    const DIMS: usize = 1;
    const VERSION: u32 = 1;

    #[inline]
    fn dim(&self, dim: usize) -> u64 {
        match dim {
            0 => self.email_hash(),
            _ => unreachable!(),
        }
    }

    fn decode<B: bytes::Buf>(version: u32, dims: &[u64], buf: B) -> std::io::Result<Self> {
        match version {
            1 => {
                let mut inner: libaccount::Unverified<VerifyCx> =
                    bincode::deserialize_from(buf.reader())
                        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
                unsafe { inner.initialize_email_hash(dims[0]) };
                Ok(Self { inner })
            }
            _ => unreachable!("unsupported data version {version}"),
        }
    }

    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: B) -> std::io::Result<()> {
        bincode::serialize_into(buf.writer(), &self.inner)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
    }
}

impl Deref for Unverified {
    type Target = libaccount::Unverified<VerifyCx>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Unverified {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl From<Unverified> for libaccount::Unverified<VerifyCx> {
    #[inline]
    fn from(val: Unverified) -> Self {
        val.inner
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

impl libaccount::ExtVerify<()> for VerifyCx {
    type Args = Captcha;
    type Error = Error;

    fn into_verified_ext(
        self,
        args: &libaccount::VerifyDescriptor<Self::Args>,
    ) -> Result<(), Self::Error> {
        // Validate the captcha.
        if self.captcha == args.ext_args {
            Ok(())
        } else {
            Err(Error::CaptchaIncorrect)
        }
    }
}
