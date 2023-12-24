use std::{
    fmt::Display,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

/// A department.
///
/// # dmds Integration
///
/// Make sure the `id` field was **initialized** when storing a newly
/// created department.
///
/// See [`Self::initialize_id`].
#[derive(Debug, Eq, Clone)]
pub struct Department {
    id: u64,
    val: String,
}

impl Department {
    #[inline]
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl Department {
    /// Initializes the inner hash of this department.
    pub fn initialize_id(&mut self) {
        let mut hasher = siphasher::sip::SipHasher24::new();
        self.val.hash(&mut hasher);
        self.id = hasher.finish();
    }
}

impl Hash for Department {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.val.hash(state)
    }
}

impl PartialEq for Department {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}

impl dmds::Data for Department {
    const DIMS: usize = 1;
    const VERSION: u32 = 1;

    fn dim(&self, dim: usize) -> u64 {
        match dim {
            0 => self.id,
            _ => unreachable!(),
        }
    }

    fn decode<B: bytes::Buf>(version: u32, dims: &[u64], buf: B) -> std::io::Result<Self> {
        match version {
            1 => bincode::deserialize_from::<_, String>(buf.reader())
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
                .map(|val| Self { id: dims[0], val }),
            _ => unreachable!("unsupported data version {version}"),
        }
    }

    #[inline]
    fn encode<B: bytes::BufMut>(&self, buf: B) -> std::io::Result<()> {
        bincode::serialize_into(buf.writer(), &self.val)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
    }
}

impl Display for Department {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.val)
    }
}

impl AsRef<str> for Department {
    fn as_ref(&self) -> &str {
        &self.val
    }
}

impl From<String> for Department {
    #[inline]
    fn from(value: String) -> Self {
        Self { id: 0, val: value }
    }
}

impl Serialize for Department {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.val)
    }
}

impl<'de> Deserialize<'de> for Department {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(From::from)
    }
}
