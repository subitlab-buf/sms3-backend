use hyper::StatusCode;
use image::DynamicImage;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ops::DerefMut,
    sync::atomic::{AtomicBool, Ordering},
};

pub static INSTANCE: Lazy<CacheManager> = Lazy::new(CacheManager::new);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("image error: {0}")]
    Image(image::ImageError),
    #[error("image too large: {0} bytes, max 50MB")]
    ImgTooLarge(usize),
    #[error("cache not found")]
    NotFound,
}

impl crate::AsResCode for Error {
    fn response_code(&self) -> StatusCode {
        match self {
            Error::Image(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::ImgTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Error::NotFound => StatusCode::NOT_FOUND,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PostImageCache {
    pub hash: u64,
    pub uploader: u64,
    /// Indicates if this cache is blocked by a post.
    pub blocked: AtomicBool,

    /// The image cache of this cache, only used for pushing into a manager instance.
    #[serde(skip)]
    pub img: RwLock<Option<DynamicImage>>,
}

impl PostImageCache {
    /// Create a new cache and its hash from image bytes.
    pub fn new(bytes: &[u8], uploader: u64) -> Result<Self, Error> {
        {
            let len = bytes.len();
            if len > 50_000_000 {
                return Err(Error::ImgTooLarge(len));
            }
        }

        let image = image::load_from_memory(bytes).map_err(Error::Image)?;

        let hash = {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            hasher.finish()
        };

        Ok(Self {
            hash,
            uploader,
            blocked: AtomicBool::new(false),
            img: RwLock::new(Some(image)),
        })
    }

    fn save(&self) {
        #[cfg(not(test))]
        {
            let this = Self {
                hash: self.hash,
                uploader: self.uploader,
                blocked: AtomicBool::new(false),
                img: RwLock::new(self.img.write().take()),
            };

            tokio::spawn(async move {
                if let Some(img) = &this.img.read().as_ref() {
                    img.save_with_format(
                        format!("./data/images/{}.png", this.hash),
                        image::ImageFormat::Png,
                    )
                    .unwrap();
                    *this.img.write().deref_mut() = None;
                }

                use tokio::io::AsyncWriteExt;

                if let Ok(mut file) =
                    tokio::fs::File::create(format!("./data/images/{}.toml", this.hash)).await
                {
                    file.write_all(toml::to_string(&this).unwrap().as_bytes())
                        .await
                        .unwrap()
                }
            });
        }

        #[cfg(test)]
        {
            *self.img.write().deref_mut() = None;
        }
    }
}

pub struct CacheManager {
    pub caches: RwLock<Vec<PostImageCache>>,
}

impl CacheManager {
    const MAX_UNBLOCKED_CACHE: usize = 64;

    pub fn new() -> Self {
        #[cfg(not(test))]
        {
            use std::fs::File;
            use std::io::Read;

            let mut vec = Vec::new();
            for dir in std::fs::read_dir("./data/images").unwrap() {
                if let Ok(f) = dir {
                    if let Ok(cache) = {
                        toml::from_str::<PostImageCache>(&{
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
                caches: RwLock::new(vec),
            }
        }

        #[cfg(test)]
        Self {
            caches: RwLock::new(Vec::new()),
        }
    }

    /// Push and save a cache.
    pub fn push(&self, cache: PostImageCache) {
        let cr = self.caches.read();

        if cr.iter().any(|e| e.hash == cache.hash) {
            return;
        }

        if Self::MAX_UNBLOCKED_CACHE
            <= cr
                .iter()
                .map(|c| {
                    if c.blocked.load(Ordering::Acquire) {
                        0
                    } else {
                        1
                    }
                })
                .sum()
        {
            let mut i = 0;
            for e in cr.iter().enumerate() {
                if !e.1.blocked.load(Ordering::Acquire) {
                    let _ = std::fs::remove_file(format!("./data/images/{}.png", e.1.hash));
                    i = e.0;
                    break;
                }
            }
            drop(cr);
            self.caches.write().remove(i);
        } else {
            drop(cr)
        }

        cache.save();
        self.caches.write().push(cache);
    }

    #[cfg(test)]
    pub fn reset(&self) {
        *self.caches.write().deref_mut() = Vec::new();
    }
}
