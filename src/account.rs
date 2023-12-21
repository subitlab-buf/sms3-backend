use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::Error;

/// A permission group of an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Permission {
    /// Contains all permissions.
    Op,
    /// Allows the user to post postings.
    Post,
    /// Allows the user to get public posts.
    GetPublic,
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
            (Permission::Op, _) | (Permission::Post, Permission::GetPublic)
        )
    }
}

/// A verified account.
#[derive(Debug)]
pub struct Account {
    inner: libaccount::Account<Permission>,
}

impl From<libaccount::Account<Permission>> for Account {
    #[inline]
    fn from(inner: libaccount::Account<Permission>) -> Self {
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
                let mut inner: libaccount::Account<Permission> =
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
    type Target = libaccount::Account<Permission>;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyCx {
    /// The captcha.
    captcha: Captcha,
    /// The time of the last captcha-send request.
    #[serde(with = "time::serde::timestamp")]
    last_req: OffsetDateTime,
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
pub struct Captcha(u16);

impl libaccount::ExtVerify<()> for VerifyCx {
    type Args = Captcha;
    type Error = Error;

    fn into_verified_ext(
        self,
        args: &libaccount::VerifyDescriptor<Self::Args>,
    ) -> Result<(), Self::Error> {
        if self.captcha == args.ext_args {
            Ok(())
        } else {
            Err(Error::CaptchaIncorrect)
        }
    }
}
