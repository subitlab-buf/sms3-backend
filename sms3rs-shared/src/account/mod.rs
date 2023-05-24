pub mod handle;

use serde::{Deserialize, Serialize};

/// Represents houses of PKUSchool.
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum House {
    GeWu,
    ZhiZhi,
    ChengYi,
    ZhengXin,
    MingDe,
    HongYi,
    XiJing,
    XinMin,
    ZhiShan,
}

/// Represents a user's metadata.
#[derive(Serialize, Deserialize)]
pub struct UserMetadata {
    pub email: lettre::Address,
    pub name: String,
    pub school_id: u32,
    pub phone: u64,
    pub house: Option<House>,
    pub organization: Option<String>,
}

pub type Permissions = Vec<Permission>;

/// Represent permissions an account has.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Permission {
    /// View approved and currently active posters.
    View,
    /// Post posters for approval.
    Post,
    /// View all posters, including archived and unapproved.
    Check,
    /// Approve posters or edit approvals.
    Approve,
    /// View accounts.
    ViewAccounts,
    /// Manage accounts.
    ManageAccounts,
    /// The top OP permission. No usage.
    OP,
}
