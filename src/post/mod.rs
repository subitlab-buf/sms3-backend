mod cache;

use chrono::{DateTime, NaiveDate, Utc};
use image::ImageError;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, error::Error, fmt::Display};

#[derive(Debug)]
pub enum PostError {
    ImageError(ImageError),
}

impl Display for PostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PostError::ImageError(err) => err.fmt(f),
        }
    }
}

impl Error for PostError {}

/// Represents a post posted by a user.
#[derive(Serialize, Deserialize, Debug)]
pub struct Post {
    /// The only id of this post.
    id: u64,
    /// File hashes of images.
    images: Vec<u64>,
    /// The status of this post (including history statuses).
    status: VecDeque<PostAcceptationData>,
    metadata: PostMetadata,
}

#[derive(Serialize, Deserialize, Debug)]
struct PostMetadata {
    title: String,
    description: String,
    /// Time range to display of this post.
    time_range: (NaiveDate, NaiveDate),
}

#[derive(Serialize, Deserialize, Debug)]
struct PostAcceptationData {
    /// Permitter of the acceptation, stored with account id.
    operator: u64,
    status: PostAcceptationStatus,
    /// Permit time.
    time: DateTime<Utc>,
}

/// Describes status of a post.
#[derive(Serialize, Deserialize, Debug)]
pub enum PostAcceptationStatus {
    Accepted(String),
    Pending,
    Rejected(String),
}

impl Default for PostAcceptationStatus {
    fn default() -> Self {
        Self::Pending
    }
}
