//! Size and time limits as types, so a zero or absurd value cannot be built.

use std::time::Duration;

/// A byte budget of at least one byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteLimit(usize);

impl ByteLimit {
    pub const fn new(bytes: usize) -> Self {
        Self(if bytes == 0 { 1 } else { bytes })
    }

    pub const fn get(self) -> usize {
        self.0
    }

    /// The limit a caller asked for, kept between one byte and `max`.
    pub fn requested(asked: Option<u64>, default: Self, max: Self) -> Self {
        let Some(asked) = asked else {
            return default;
        };
        let asked = usize::try_from(asked).unwrap_or(usize::MAX);
        Self(asked.clamp(1, max.0))
    }
}

/// A time budget of at least one millisecond.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeout(Duration);

impl Timeout {
    pub const fn from_secs(secs: u64) -> Self {
        Self(Duration::from_secs(if secs == 0 { 1 } else { secs }))
    }

    pub fn get(self) -> Duration {
        self.0
    }

    /// The timeout a caller asked for in milliseconds, kept between 1 ms and `max`.
    pub fn requested_ms(asked: Option<u64>, default: Self, max: Self) -> Self {
        let Some(asked) = asked else {
            return default;
        };
        Self(Duration::from_millis(asked.max(1)).min(max.0))
    }
}
