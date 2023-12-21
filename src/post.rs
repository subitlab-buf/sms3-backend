use std::ops::RangeInclusive;

#[derive(Debug)]
pub struct Post {
    id: u64,
    title: String,
    /// On-screen time range.
    dates: RangeInclusive<time::Date>,
    /// Descriptions about this post.
    notes: String,
}
