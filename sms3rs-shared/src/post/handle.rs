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

#[derive(Serialize, Deserialize)]
pub enum GetPostsFilter {
    /// Posts that match target status.
    Acceptation(super::PostAcceptationStatus),
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

#[derive(Serialize, Deserialize)]
pub enum EditPostVariant {
    Title(String),
    Description(String),
    Images(Vec<u64>),
    TimeRange(chrono::NaiveDate, chrono::NaiveDate),
    /// Change status of the post to `Pending`
    /// if the target status is `Submitted`.
    CancelSubmission,
    RequestReview(
        /// Message to admins.
        String,
    ),
    /// Remove the post and unblock all the images it use.
    Destroy,
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

#[derive(Serialize, Deserialize)]
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
