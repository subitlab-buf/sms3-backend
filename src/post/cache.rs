use super::PostError;
use chrono::{NaiveDateTime, Utc};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    format, fs,
    hash::{Hash, Hasher},
};
use tide::log::error;

#[derive(Serialize, Deserialize)]
pub struct PostImageCache {
    hash: u64,
    creation_time: NaiveDateTime,
    /// Indicates if this cache is blocked by a post.
    blocked: bool,

    /// The image cache of this cache, only used for pushing into a manager instance.
    #[serde(skip)]
    img: Option<DynamicImage>,
}

impl PostImageCache {
    pub fn new(bytes: &Vec<u8>) -> Result<Self, PostError> {
        let image = image::load_from_memory(&bytes).map_err(|err| PostError::ImageError(err))?;
        let hash = {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            hasher.finish()
        };
        Ok(Self {
            hash,
            creation_time: Utc::now().naive_utc(),
            blocked: false,
            img: Some(image),
        })
    }

    fn save(&mut self) -> bool {
        match &self.img {
            Some(img) => {
                let ok = img
                    .save_with_format(
                        format!("./data/images/{}.png", self.hash),
                        image::ImageFormat::Png,
                    )
                    .is_ok();
                drop(img);
                self.img = None;
                ok
            }
            None => false,
        }
    }
}

pub struct CacheManager {
    caches: Vec<PostImageCache>,
}

impl CacheManager {
    const MAX_UNBLOCKED_CACHE: usize = 64;

    pub fn push(&mut self, mut cache: PostImageCache) {
        if self.caches.iter().any(|e| e.hash == cache.hash) {
            return;
        }
        if Self::MAX_UNBLOCKED_CACHE
            <= self
                .caches
                .iter()
                .map(|c| if c.blocked { 0 } else { 1 })
                .sum()
        {
            let mut i = 0;
            for e in self.caches.iter().enumerate() {
                if !e.1.blocked {
                    let _ = fs::remove_file(format!("./data/images/{}.png", e.1.hash));
                    i = e.0;
                    break;
                }
            }
            self.caches.remove(i);
        }
        if !cache.save() {
            error!("Image cache {} save failed", cache.hash);
        }
        self.caches.push(cache);
    }
}
