use super::PostError;
use async_std::sync::RwLock;
use image::DynamicImage;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    ops::DerefMut,
    sync::atomic::{AtomicBool, Ordering},
};
use tide::log::error;

pub static INSTANCE: Lazy<CacheManager> = Lazy::new(CacheManager::new);

#[derive(Serialize, Deserialize)]
pub struct PostImageCache {
    pub(super) hash: u64,
    uploader: u64,
    /// Indicates if this cache is blocked by a post.
    pub(super) blocked: AtomicBool,

    /// The image cache of this cache, only used for pushing into a manager instance.
    #[serde(skip)]
    img: RwLock<Option<DynamicImage>>,
}

impl PostImageCache {
    /// Create a new cache and its hash from image bytes.
    pub fn new(bytes: &Vec<u8>, uploader: u64) -> Result<(Self, u64), PostError> {
        let image = image::load_from_memory(bytes).map_err(PostError::ImageError)?;
        let hash = {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            hasher.finish()
        };
        Ok((
            Self {
                hash,
                uploader,
                blocked: AtomicBool::new(false),
                img: RwLock::new(Some(image)),
            },
            hash,
        ))
    }

    #[cfg(not(test))]
    #[must_use = "The save result should be handled"]
    async fn save(&self) -> bool {
        use std::fs::File;
        use std::io::Write;
        use std::ops::Deref;

        (match &self.img.read().await.deref() {
            Some(img) => {
                let ok = img
                    .save_with_format(
                        format!("./data/images/{}.png", self.hash),
                        image::ImageFormat::Png,
                    )
                    .is_ok();
                *self.img.write().await.deref_mut() = None;
                ok
            }
            None => true,
        }) && (match File::create(format!("./data/images/{}.toml", self.hash)) {
            Ok(mut file) => file
                .write_all(
                    match toml::to_string(self) {
                        Ok(e) => e,
                        Err(_) => return false,
                    }
                    .as_bytes(),
                )
                .is_ok(),
            Err(_) => false,
        })
    }

    #[cfg(test)]
    #[must_use = "The save result should be handled"]
    async fn save(&self) -> bool {
        *self.img.write().await.deref_mut() = None;
        true
    }
}

pub struct CacheManager {
    pub(super) caches: RwLock<Vec<PostImageCache>>,
}

impl CacheManager {
    const MAX_UNBLOCKED_CACHE: usize = 64;

    pub fn new() -> Self {
        #[cfg(not(test))]
        {
            use std::fs::File;
            use std::io::Read;

            let mut vec = Vec::new();
            for dir in fs::read_dir("./data/images").unwrap() {
                match dir {
                    Ok(f) => match {
                        toml::from_str::<PostImageCache>(&{
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
                caches: RwLock::new(vec),
            }
        }

        #[cfg(test)]
        Self {
            caches: RwLock::new(Vec::new()),
        }
    }

    /// Push and save a cache.
    pub async fn push(&self, cache: PostImageCache) {
        let cr = self.caches.read().await;
        if cr.iter().any(|e| e.hash == cache.hash) {
            return;
        }
        if Self::MAX_UNBLOCKED_CACHE
            <= cr
                .iter()
                .map(|c| {
                    if c.blocked.load(Ordering::Relaxed) {
                        0
                    } else {
                        1
                    }
                })
                .sum()
        {
            let mut i = 0;
            for e in cr.iter().enumerate() {
                if !e.1.blocked.load(Ordering::Relaxed) {
                    let _ = fs::remove_file(format!("./data/images/{}.png", e.1.hash));
                    i = e.0;
                    break;
                }
            }
            drop(cr);
            self.caches.write().await.remove(i);
        } else {
            drop(cr)
        }
        if !cache.save().await {
            error!("Image cache {} save failed", cache.hash);
        }
        self.caches.write().await.push(cache);
    }
}
