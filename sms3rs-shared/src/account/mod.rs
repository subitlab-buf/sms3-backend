pub mod handle;

use serde::{Deserialize, Serialize};

/// Represents houses of PKUSchool.
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq)]
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
    /// Approve posters or edit approvals.
    Approve,
    /// View all posters, including archived and unapproved.
    Check,
    ManageAccounts,
    /// The top permission, no actual usage.
    OP,
    /// Post posters for approval.
    Post,
    /// View approved and currently active posters.
    View,
    ViewAccounts,
}
