pub(crate) mod cache;
pub mod handle;

use once_cell::sync::Lazy;
use parking_lot::RwLock;

pub use sms3rs_shared::post::*;

pub static INSTANCE: Lazy<PostManager> = Lazy::new(PostManager::new);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cache error: {0}")]
    Cache(cache::Error),
    #[error("post id conflicted")]
    Conflict,
    #[error("post lifetime out of range (longest: 7 days)")]
    DateOutOfRange,
    #[error("post not found")]
    NotFound,
    #[error("post already in status: {0:?}")]
    Already(PostAcceptationStatus),
    #[error("status update message couldn't be empty")]
    MsgEmpty,
}

impl crate::AsResCode for Error {
    fn response_code(&self) -> hyper::StatusCode {
        match self {
            Error::Cache(err) => err.response_code(),
            Error::Conflict => hyper::StatusCode::CONFLICT,
            _ => hyper::StatusCode::FORBIDDEN,
        }
    }
}

pub fn save_post(_post: &Post) {
    #[cfg(not(test))]
    {
        let this = _post.clone();

        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;

            if let Ok(mut file) =
                tokio::fs::File::create(format!("./data/posts/{}.toml", this.id)).await
            {
                file.write_all(toml::to_string(&this).unwrap().as_bytes())
                    .await
                    .unwrap()
            }
        })
    };
}

pub fn remove_post(_post: &Post) {
    #[cfg(not(test))]
    {
        let id = _post.id;

        tokio::spawn(async move {
            tokio::fs::remove_file(format!("./data/posts/{}.toml", id))
                .await
                .unwrap()
        });
    }
}

pub struct PostManager {
    pub posts: RwLock<Vec<RwLock<Post>>>,
}

impl PostManager {
    fn new() -> Self {
        #[cfg(not(test))]
        {
            use std::fs::{self, File};
            use std::io::Read;

            let mut vec = Vec::new();

            for dir in fs::read_dir("./data/posts").unwrap() {
                if let Ok(f) = dir {
                    if let Ok(cache) = {
                        toml::from_str::<Post>(&{
                            let mut string = String::new();
                            File::open(f.path())
                                .unwrap()
                                .read_to_string(&mut string)
                                .unwrap();
                            string
                        })
                    } {
                        vec.push(cache)
                    }
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

        #[cfg(test)]
        Self {
            posts: RwLock::new(Vec::new()),
        }
    }

    pub fn push(&self, post: Post) {
        self.posts.write().push(RwLock::new(post))
    }

    /// Indicates if the target id is already contained in this instance.
    pub fn contains_id(&self, id: u64) -> bool {
        self.posts.read().iter().any(|e| e.read().id == id)
    }

    #[cfg(test)]
    pub fn reset(&self) {
        *std::ops::DerefMut::deref_mut(&mut self.posts.write()) = Vec::new();
    }
}
