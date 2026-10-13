//! A one-value TTL cache with single flight: callers that arrive while a fetch is in
//! progress wait for it and share its result instead of starting their own.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

pub struct Cached<T> {
    /// How long a value stays fresh, decided per value: a failure should be retried
    /// sooner than a good answer needs refreshing.
    ttl: fn(&T) -> Duration,
    slot: Mutex<Option<(Instant, T)>>,
    /// Bumped on every completed fetch, so a caller that queued behind one can tell it
    /// already has a result as new as anything it could fetch itself.
    generation: AtomicU64,
}

impl<T: Clone> Cached<T> {
    pub fn new(ttl: fn(&T) -> Duration) -> Self {
        Self {
            ttl,
            slot: Mutex::new(None),
            generation: AtomicU64::new(0),
        }
    }

    /// The cached value while it is fresh, otherwise the result of `fetch`. `force`
    /// skips the freshness check, but still shares a fetch that finished while this
    /// caller was waiting for the lock.
    pub async fn get<F, Fut>(&self, force: bool, fetch: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let seen = self.generation.load(Ordering::Acquire);
        let mut slot = self.slot.lock().await;
        if let Some((stored, value)) = slot.as_ref() {
            let shared = self.generation.load(Ordering::Acquire) != seen;
            let fresh = !force && stored.elapsed() < (self.ttl)(value);
            if shared || fresh {
                return value.clone();
            }
        }
        let value = fetch().await;
        *slot = Some((Instant::now(), value.clone()));
        self.generation.fetch_add(1, Ordering::AcqRel);
        value
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
