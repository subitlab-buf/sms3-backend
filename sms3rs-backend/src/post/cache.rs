use std::{
    hash::{Hash, Hasher},
    ops::DerefMut,
};

pub static INSTANCE: once_cell::sync::Lazy<Caches> = once_cell::sync::Lazy::new(Caches::new);

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PostImageCache {
    pub hash: u64,
    pub uploader: u64,
    /// Indicates if this cache is blocked by a post.
    pub blocked: std::sync::atomic::AtomicBool,

    /// The image cache of this cache, only used for pushing into a manager instance.
    #[serde(skip)]
    pub img: tokio::sync::RwLock<Option<image::DynamicImage>>,
}

impl PostImageCache {
    /// Create a new cache and its hash from image bytes.
    pub fn new(bytes: &[u8], uploader: u64) -> anyhow::Result<(Self, u64)> {
        let image = image::load_from_memory(bytes)?;
        let hash = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            bytes.hash(&mut hasher);
            hasher.finish()
        };
        Ok((
            Self {
                hash,
                uploader,
                blocked: std::sync::atomic::AtomicBool::new(false),
                img: tokio::sync::RwLock::new(Some(image)),
            },
            hash,
        ))
    }

    #[cfg(not(test))]
    #[must_use = "The save result should be handled"]
    async fn save(&self) -> bool {
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
        }) && (match tokio::fs::File::create(format!("./data/images/{}.toml", self.hash)).await {
            Ok(mut file) => tokio::io::AsyncWriteExt::write_all(
                &mut file,
                match toml::to_string(self) {
                    Ok(e) => e,
                    Err(_) => return false,
                }
                .as_bytes(),
            )
            .await
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

pub struct Caches {
    pub caches: tokio::sync::RwLock<Vec<PostImageCache>>,
}

impl Caches {
    const MAX_UNBLOCKED_CACHE: usize = 64;

    pub fn new() -> Self {
        #[cfg(not(test))]
        {
            use std::fs::File;
            use std::io::Read;

            let mut vec = Vec::new();
            for dir in std::fs::read_dir("./data/images").unwrap() {
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
                caches: tokio::sync::RwLock::new(vec),
            }
        }

        #[cfg(test)]
        Self {
            caches: tokio::sync::RwLock::new(Vec::new()),
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
                    if c.blocked.load(std::sync::atomic::Ordering::Relaxed) {
                        0
                    } else {
                        1
                    }
                })
                .sum()
        {
            let mut i = 0;
            for e in cr.iter().enumerate() {
                if !e.1.blocked.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = tokio::fs::remove_file(format!("./data/images/{}.png", e.1.hash)).await;
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
            tracing::error!("Image cache {} save failed", cache.hash);
        }
        self.caches.write().await.push(cache);
    }

    #[cfg(test)]
    pub async fn reset(&self) {
        *self.caches.write().await.deref_mut() = Vec::new();
    }
}
