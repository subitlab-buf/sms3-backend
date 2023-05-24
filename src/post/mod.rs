mod cache;
pub mod handle;

use async_std::sync::RwLock;
use chrono::{DateTime, NaiveDate, Utc};
use image::ImageError;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    error::Error,
    fmt::Display,
    fs::{self, File},
    io::{Read, Write},
};

pub static INSTANCE: Lazy<PostManager> = Lazy::new(PostManager::new);

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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Post {
    /// The only id of this post.
    id: u64,
    /// File hashes of images.
    images: Vec<u64>,
    metadata: PostMetadata,
    /// The requester of this post in user id.
    publisher: u64,
    /// The status of this post (including history status inside a deque).
    /// Newer status will be pushed to back of the deque.
    status: VecDeque<PostAcceptationData>,
}

impl Post {
    #[must_use = "The save result should be handled"]
    pub fn save(&self) -> bool {
        match File::create(format!("./data/posts/{}.toml", self.id)) {
            Ok(mut file) => file
                .write_all(
                    match toml::to_string(self) {
                        Ok(s) => s,
                        Err(_) => return false,
                    }
                    .as_bytes(),
                )
                .is_ok(),
            Err(_) => false,
        }
    }

    #[must_use = "The deletion result should be handled"]
    pub fn remove(&self) -> bool {
        fs::remove_file(format!("./data/posts/{}.toml", self.id)).is_ok()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PostMetadata {
    title: String,
    /// Description of this post, should be secret to users except admins and publisher.
    description: String,
    /// Time range to display of this post.
    time_range: (NaiveDate, NaiveDate),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PostAcceptationData {
    /// Operator of the acceptation, stored with account id.
    operator: u64,
    status: PostAcceptationStatus,
    /// Operate time.
    time: DateTime<Utc>,
}

/// Describes status of a post.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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

impl Default for PostAcceptationStatus {
    fn default() -> Self {
        Self::Pending
    }
}

pub struct PostManager {
    posts: RwLock<Vec<RwLock<Post>>>,
}

impl PostManager {
    fn new() -> Self {
        let mut vec = Vec::new();
        for dir in fs::read_dir("./data/posts").unwrap() {
            match dir {
                Ok(f) => match {
                    toml::from_str::<Post>(&{
                        let mut string = String::new();
                        File::open(f.path())
                            .unwrap()
                            .read_to_string(&mut string)
                            .unwrap();
                        string
                    })
                } {
                    Ok(cache) => vec.push(cache),
                    Err(_) => (),
                },
                Err(_) => (),
            }
        }
        Self {
            posts: {
                let mut v = Vec::new();
                for e in vec {
                    v.push(RwLock::new(e));
                }
                RwLock::new(v)
            },
        }
    }

    pub async fn push(&self, post: Post) {
        self.posts.write().await.push(RwLock::new(post))
    }

    /// Indicates if the target id is already contained in this instance.
    pub async fn contains_id(&self, id: u64) -> bool {
        for post in self.posts.read().await.iter() {
            if post.read().await.id == id {
                return true;
            }
        }
        false
    }
}
