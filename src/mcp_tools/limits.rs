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

/// What happens to a command that runs long.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunLimit {
    /// Kill it at this point.
    Kill(Timeout),
    /// Move it to the background at `after`. If `kill_at` is set, kill it that long after
    /// it started.
    Background {
        after: Timeout,
        kill_at: Option<Timeout>,
    },
}

impl RunLimit {
    /// No timeout means "background it at `after`". A timeout up to `after` is a hard kill.
    /// A longer one still backgrounds at `after`, then kills at the timeout, kept to `hard_max`.
    pub fn requested_ms(asked: Option<u64>, after: Timeout, hard_max: Timeout) -> Self {
        let Some(ms) = asked else {
            return Self::Background {
                after,
                kill_at: None,
            };
        };
        if Duration::from_millis(ms) <= after.get() {
            return Self::Kill(Timeout::requested_ms(Some(ms), after, after));
        }
        Self::Background {
            after,
            kill_at: Some(Timeout::requested_ms(Some(ms), hard_max, hard_max)),
        }
    }

    /// How long the command runs before this limit acts.
    pub fn first_deadline(self) -> Duration {
        match self {
            Self::Kill(timeout) => timeout.get(),
            Self::Background { after, .. } => after.get(),
        }
    }
}
