use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct Resource {
    #[serde(skip)]
    id: u64,
    variant: Variant,

    #[serde(with = "time::serde::timestamp")]
    creation_time: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Image,
    Pdf,
    Video,
}
