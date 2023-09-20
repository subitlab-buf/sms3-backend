use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GetImageDescriptor {
    pub hash: u64,
}

#[derive(Serialize, Deserialize)]
pub struct PostDescriptor {
    pub title: String,
    pub description: String,
    pub time_range: (chrono::NaiveDate, chrono::NaiveDate),
    pub images: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct GetPostsDescriptor {
    pub filters: Vec<GetPostsFilter>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum SimplePostAcceptationStatus {
    Accepted,
    Pending,
    Rejected,
    Submitted,
}

impl SimplePostAcceptationStatus {
    pub fn matches(&self, status: &super::PostAcceptationStatus) -> bool {
        match self {
            SimplePostAcceptationStatus::Accepted => {
                matches!(status, super::PostAcceptationStatus::Accepted(_))
            }
            SimplePostAcceptationStatus::Pending => {
                matches!(status, super::PostAcceptationStatus::Pending)
            }
            SimplePostAcceptationStatus::Rejected => {
                matches!(status, super::PostAcceptationStatus::Rejected(_))
            }
            SimplePostAcceptationStatus::Submitted => {
                matches!(status, super::PostAcceptationStatus::Submitted(_))
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum GetPostsFilter {
    /// Posts that match target status.
    Acceptation(SimplePostAcceptationStatus),
    /// Posts published by target account.
    Account(u64),
    After(chrono::NaiveDate),
    Before(chrono::NaiveDate),
    /// Posts which their title and description contains target keywords.
    Keyword(String),
}

#[derive(Serialize, Deserialize)]
pub struct EditPostDescriptor {
    pub post: u64,
    pub variants: Vec<EditPostVariant>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum EditPostVariant {
    /// Change status of the post to `Pending`
    /// if the target status is `Submitted`.
    CancelSubmission,
    Description(String),
    /// Remove the post and unblock all the images it use.
    Destroy,
    Images(Vec<u64>),
    RequestReview(
        /// Message to admins.
        String,
    ),
    TimeRange(chrono::NaiveDate, chrono::NaiveDate),
    Title(String),
}

#[derive(Serialize, Deserialize)]
pub struct GetPostsInfoDescriptor {
    pub posts: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
pub enum GetPostInfoResult {
    Full(super::Post),
    Foreign {
        id: u64,
        images: Vec<u64>,
        title: String,
        archived: bool,
    },
    NotFound(
        /// Target post id
        u64,
    ),
}

#[derive(Serialize, Deserialize)]
pub struct ApprovePostDescriptor {
    pub post: u64,
    pub variant: ApprovePostVariant,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ApprovePostVariant {
    Accept(
        /// Message
        Option<String>,
    ),
    Reject(
        /// Message, should not be empty.
        String,
    ),
}
