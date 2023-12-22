use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug)]
pub struct Post {
    id: u64,
    title: String,
    /// On-screen time range.
    dates: RangeInclusive<time::Date>,

    /// Post states in time order.\
    /// There should be at least one state in a post.
    states: Vec<State>,
}

impl Post {
    /// Gets the overall states of this post.
    #[inline]
    pub fn states(&self) -> &[State] {
        &self.states
    }

    /// The current state of this post.
    #[inline]
    pub fn state(&self) -> &State {
        self.states
            .last()
            .expect("there should be at least one state in a post")
    }

    #[inline]
    pub fn creator(&self) -> u64 {
        self.states
            .first()
            .expect("there should be at least one state in a post")
            .operator
    }
}

/// State of a [`Post`].
#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    status: Status,
    #[serde(with = "time::serde::timestamp")]
    time: OffsetDateTime,
    operator: u64,

    /// Description of this state.
    message: String,
}

impl State {
    /// [`Status`] of this state.
    #[inline]
    pub fn status(&self) -> Status {
        self.status
    }

    /// Creation time of this state.
    #[inline]
    pub fn time(&self) -> OffsetDateTime {
        self.time
    }

    /// Creator of this state.
    #[inline]
    pub fn operator(&self) -> u64 {
        self.operator
    }

    /// Description of this state, written by operators.
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Status {
    Pending,
    Approved,
    Rejected,
}
