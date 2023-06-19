pub(crate) mod cache;
pub mod handle;

pub use sms3rs_shared::post::*;

pub static INSTANCE: once_cell::sync::Lazy<Posts> = once_cell::sync::Lazy::new(Posts::new);

#[must_use = "The save result should be handled"]
pub async fn save_post(_post: &Post) -> bool {
    #[cfg(not(test))]
    {
        match tokio::fs::File::create(format!("./data/posts/{}.toml", _post.id)).await {
            Ok(mut file) => tokio::io::AsyncWriteExt::write_all(
                &mut file,
                match toml::to_string(_post) {
                    Ok(s) => s,
                    Err(_) => return false,
                }
                .as_bytes(),
            )
            .await
            .is_ok(),
            Err(_) => false,
        }
    }

    #[cfg(test)]
    true
}

#[must_use = "The deletion result should be handled"]
pub async fn remove_post(_post: &Post) -> bool {
    #[cfg(not(test))]
    {
        return tokio::fs::remove_file(format!("./data/posts/{}.toml", _post.id))
            .await
            .is_ok();
    }

    #[cfg(test)]
    true
}

pub struct Posts {
    pub posts: tokio::sync::RwLock<Vec<tokio::sync::RwLock<Post>>>,
}

impl Posts {
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
                        v.push(tokio::sync::RwLock::new(e));
                    }
                    tokio::sync::RwLock::new(v)
                },
            }
        }

        #[cfg(test)]
        Self {
            posts: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    pub async fn push(&self, post: Post) {
        self.posts
            .write()
            .await
            .push(tokio::sync::RwLock::new(post))
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
