use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::{Instant, OffsetDateTime};

#[derive(Debug, Serialize, Deserialize)]
pub struct Resource {
    #[serde(skip)]
    id: u64,
    variant: Variant,
    user: u64,

    #[serde(skip)]
    used: bool,
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

    /// Expire time of a session.
    const EXPIRE_TIME: time::Duration = time::Duration::seconds(15);

    /// Whether this session is expired.
    #[inline]
    fn is_expired(&self) -> bool {
        self.instant.elapsed() > Self::EXPIRE_TIME
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

    /// Inserts a new session.
    #[inline]
    pub fn insert(&mut self, res: Resource) {
        self.inner.insert(res.id, res.into());
    }

    /// Accepts the body of a resource with given id,
    /// and returns the resource.
    ///
    /// **Id of the resource** will be changed, you have to
    /// tell the new id to frontend.
    pub fn accept(&mut self, id: u64, body: &[u8], user: u64) -> Option<Resource> {}
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Image,
    Pdf,
    Video,
}
