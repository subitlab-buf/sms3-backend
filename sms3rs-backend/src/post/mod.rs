pub(crate) mod cache;
pub mod handle;

use async_std::sync::RwLock;
use image::ImageError;
use once_cell::sync::Lazy;
use std::{error::Error, fmt::Display};

pub use sms3rs_shared::post::*;

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

#[must_use = "The save result should be handled"]
pub fn save_post(_post: &Post) -> bool {
    #[cfg(not(test))]
    {
        use std::fs::File;
        use std::io::Write;

        match File::create(format!("./data/posts/{}.toml", _post.id)) {
            Ok(mut file) => file
                .write_all(
                    match toml::to_string(_post) {
                        Ok(s) => s,
                        Err(_) => return false,
                    }
                    .as_bytes(),
                )
                .is_ok(),
            Err(_) => false,
        }
    }

    #[cfg(test)]
    true
}

#[must_use = "The deletion result should be handled"]
pub fn remove_post(_post: &Post) -> bool {
    #[cfg(not(test))]
    {
        use std::fs;
        return fs::remove_file(format!("./data/posts/{}.toml", _post.id)).is_ok();
    }

    #[cfg(test)]
    true
}

pub struct PostManager {
    posts: RwLock<Vec<RwLock<Post>>>,
}

impl PostManager {
    fn new() -> Self {
        #[cfg(not(test))]
        {
            use std::fs::{self, File};
            use std::io::Read;

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

        #[cfg(test)]
        Self {
            posts: RwLock::new(Vec::new()),
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

    #[cfg(test)]
    pub async fn reset(&self) {
        use std::ops::DerefMut;

        *self.posts.write().await.deref_mut() = Vec::new();
    }
}
