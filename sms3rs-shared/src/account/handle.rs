use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EmailTarget {
    pub email: lettre::Address,
}

#[derive(Serialize, Deserialize)]
pub struct AccountVerifyDescriptor {
    pub code: u32,
    pub variant: AccountVerifyVariant,
}

#[derive(Serialize, Deserialize)]
pub enum AccountVerifyVariant {
    /// Activate an unverified account.
    Activate {
        name: String,
        id: u32,
        phone: u64,
        house: Option<super::House>,
        organization: Option<String>,
        password: String,
    },
    /// Verify a resetpassword session.
    ResetPassword(String),
}

#[derive(Serialize, Deserialize)]
pub struct AccountLoginDescriptor {
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct AccountSignOutDescriptor {
    /// For double-verifying.
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ViewAccountResult {
    pub id: u64,
    pub metadata: super::UserMetadata,
    pub permissions: super::Permissions,
    pub registration_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct AccountEditDescriptor {
    pub variants: Vec<AccountEditVariant>,
}

#[derive(Serialize, Deserialize)]
pub enum AccountEditVariant {
    Name(String),
    SchoolId(u32),
    Phone(u64),
    House(Option<super::House>),
    Organization(Option<String>),
    Password { old: String, new: String },
    TokenExpireTime(u16),
}

pub mod manage {
    use crate::account;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub struct MakeAccountDescriptor {
        pub email: lettre::Address,
        pub name: String,
        pub school_id: u32,
        pub phone: u64,
        pub house: Option<account::House>,
        pub organization: Option<String>,
        pub password: String,
        pub permissions: account::Permissions,
    }

    #[derive(Serialize, Deserialize)]
    pub struct ViewAccountDescriptor {
        pub accounts: Vec<u64>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum ViewAccountResult {
        Err { id: u64, error: String },
        Ok(super::ViewAccountResult),
    }

    #[derive(Serialize, Deserialize)]
    pub struct ModifyAccountDescriptor {
        pub account_id: u64,
        pub variants: Vec<AccountModifyVariant>,
    }

    #[derive(Serialize, Deserialize)]
    pub enum AccountModifyVariant {
        Email(lettre::Address),
        Name(String),
        SchoolId(u32),
        Phone(u64),
        House(Option<account::House>),
        Organization(Option<String>),
        Permission(account::Permissions),
    }
}
