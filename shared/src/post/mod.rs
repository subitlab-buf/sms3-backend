pub mod handle;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Represents a post posted by a user.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Post {
    /// The only id of this post.
    pub id: u64,
    /// File hashes of images.
    pub images: Vec<u64>,
    pub metadata: PostMetadata,
    /// The requester of this post in user id.
    pub publisher: u64,
    /// The status of this post (including history status inside a deque).
    /// Newer status will be pushed to back of the deque.
    pub status: Vec<PostAcceptationData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostMetadata {
    pub title: String,
    /// Description of this post, should be secret to users except admins and publisher.
    pub description: String,
    /// Time range to display of this post.
    pub time_range: (chrono::NaiveDate, chrono::NaiveDate),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostAcceptationData {
    /// Operator of the acceptation, stored with account id.
    pub operator: u64,
    pub status: PostAcceptationStatus,
    /// Operate time.
    pub time: chrono::DateTime<chrono::Utc>,
}

/// Describes status of a post.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PostAcceptationStatus {
    /// The post was accepted with a message.
    Accepted(String),
    /// The post is pending to be submitted,
    /// admins are not able to accept it.
    Pending,
    /// The post was rejected by an admin with a message.
    Rejected(String),
    /// The post was submitted with a message for admins by publisher.
    Submitted(String),
}
