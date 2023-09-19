mod raw;

pub use sms3rs_shared::account::{House, Permission};

pub struct AccoutInfo {
    email: String,
    user_id: u64,
    token: Option<String>,
    user: Option<std::sync::Arc<User>>,
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
