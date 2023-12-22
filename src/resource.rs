use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};
use time::{Instant, OffsetDateTime};

use crate::Error;

/// Reference and metadata of a resource file.
///
/// # dmds Dimensions
///
/// ```txt
/// 0 -> id
/// 1 -> used (false -> 0, true -> 1)
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Resource {
    #[serde(skip)]
    id: u64,
    variant: Variant,
    user: u64,

    #[serde(skip)]
    used: bool,
}

impl Resource {
    /// Creates a new resource with given variant and user.
    ///
    /// The id will be generated randomly based on the
    /// time and account.
    pub fn new(variant: Variant, account: u64) -> Self {
        let mut hasher = siphasher::sip::SipHasher24::new();
        OffsetDateTime::now_utc().hash(&mut hasher);
        account.hash(&mut hasher);
        rand::random::<i32>().hash(&mut hasher);

        Self {
            id: hasher.finish(),
            variant,
            user: account,
            used: false,
        }
    }
}

impl dmds::Data for Resource {
    const DIMS: usize = 2;
    const VERSION: u32 = 1;

    #[inline]
    fn dim(&self, dim: usize) -> u64 {
        match dim {
            0 => self.id,
            1 => self.used as u64,
            _ => unreachable!(),
        }
    }

    fn decode<B: bytes::Buf>(version: u32, dims: &[u64], buf: B) -> std::io::Result<Self> {
        match version {
            0 => {
                let mut this: Self = bincode::deserialize_from(buf.reader())
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
                this.id = dims[0];
                this.used = dims[1] != 0;
                Ok(this)
            }
            _ => unreachable!(),
        }
    }

    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: B) -> std::io::Result<()> {
        bincode::serialize_into(buf.writer(), self)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
    }
}

/// A resource uploading session.
#[derive(Debug)]
struct UploadSession {
    resource: Resource,
    instant: Instant,
}

impl UploadSession {
    /// Creates a new session.
    #[inline]
    fn new(resource: Resource) -> Self {
        Self {
            resource,
            instant: Instant::now(),
        }
    }

    /// Expire duration of a session.
    const EXPIRE_DUR: time::Duration = time::Duration::seconds(15);

    /// Whether this session is expired.
    #[inline]
    fn is_expired(&self) -> bool {
        self.instant.elapsed() > Self::EXPIRE_DUR
    }
}

impl From<Resource> for UploadSession {
    #[inline]
    fn from(value: Resource) -> Self {
        Self::new(value)
    }
}

impl From<UploadSession> for Resource {
    #[inline]
    fn from(value: UploadSession) -> Self {
        value.resource
    }
}

/// Storage of resource upload sessions.
#[derive(Debug, Default)]
pub struct UploadSessions {
    /// Id => Session.
    inner: HashMap<u64, UploadSession>,
}

impl UploadSessions {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    fn cleanup(&mut self) {
        self.inner.retain(|_, v| !v.is_expired());
    }

    /// Inserts a new session.
    pub fn insert(&mut self, res: Resource) {
        self.cleanup();
        self.inner.insert(res.id, res.into());
    }

    /// Accepts the body of a resource with given id,
    /// and returns the resource.
    ///
    /// *Id of the resource* will be changed, so you have to
    /// tell the new id to *the frontend*.
    pub fn accept(&mut self, id: u64, data: &[u8], user: u64) -> Result<Resource, Error> {
        self.cleanup();
        let res = &self
            .inner
            .get(&id)
            .ok_or(Error::ResourceUploadSessionNotFound(id))?
            .resource;
        if res.user != user {
            return Err(Error::PermissionDenied);
        }

        let mut res = self.inner.remove(&id).unwrap().resource;
        res.id =
            highway::HighwayHash::hash64(highway::PortableHash::new(highway::Key::default()), data);
        Ok(res)
    }
}

/// Type of a resource.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Image,
    Pdf,
    Video,
}
