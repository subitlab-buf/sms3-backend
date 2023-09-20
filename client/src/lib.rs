mod raw;

pub use sms3rs_shared::account::{House, Permission};

pub mod raw_shared {
    pub use sms3rs_shared::*;
}

pub struct AccoutInfo {
    email: String,
    token: Option<String>,
    user: LazyAccount,
}

pub struct User {
    email: String,
    name: String,
    school_id: u32,
    phone: u64,
    house: Option<House>,
    org: Option<String>,
    permissions: Vec<Permission>,
    registration_time: String,
}

pub struct Post {
    images: Vec<u64>,
    title: String,
    archived: bool,
    ext: Option<PostExt>,
}

struct PostExt {
    description: String,
    time: std::ops::RangeInclusive<chrono::NaiveDate>,
    publisher: LazyAccount,
    status: Vec<PostAccept>,
}

pub struct PostAccept {
    operator: LazyAccount,
    status: sms3rs_shared::post::PostAcceptationStatus,
    time: chrono::DateTime<chrono::Utc>,
}

struct LazyAccount {
    id: u64,
    user: std::sync::OnceLock<std::sync::Arc<parking_lot::RwLock<anyhow::Result<User>>>>,
}

impl LazyAccount {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            user: std::sync::OnceLock::new(),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}
