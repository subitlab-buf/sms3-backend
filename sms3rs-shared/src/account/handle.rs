use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AccountCreateDescriptor {
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
        email: lettre::Address,
        name: String,
        id: u32,
        phone: u64,
        house: Option<super::House>,
        organization: Option<String>,
        password: String,
    },
    /// Verify a resetpassword session.
    ResetPassword {
        email: lettre::Address,
        password: String,
    },
}

#[derive(Serialize, Deserialize)]
pub struct AccountLoginDescriptor {
    pub email: lettre::Address,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct AccountSignOutDescriptor {
    /// For double-verifying.
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct ViewAccountResult {
    pub id: u64,
    pub metadata: super::UserMetadata,
    pub permissions: super::Permissions,
    pub registration_time: chrono::DateTime<chrono::Utc>,
    pub registration_ip: Option<String>,
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

#[derive(Serialize, Deserialize)]
pub struct ResetPasswordDescriptor {
    pub email: lettre::Address,
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

    #[derive(Serialize, Deserialize)]
    pub enum ViewAccountResult {
        Err { id: u64, error: String },
        Ok(super::ViewAccountResult),
    }

    #[derive(Serialize, Deserialize)]
    pub struct AccountModifyDescriptor {
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
