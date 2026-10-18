//! Resource limits and cancellation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

/// Wall-clock limit when the caller names none.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// The most a caller may ask for. Below every runner's own tool ceiling.
pub const MAX_TIMEOUT: Duration = Duration::from_secs(600);
const MIN_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub timeout: Duration,
    /// Heap ceiling for the script, in bytes.
    pub memory: usize,
    /// Native stack ceiling, in bytes: bounds recursion depth.
    pub stack: usize,
    /// Largest printed return value, in bytes.
    pub max_result: usize,
    /// Tool calls one run may make in total.
    pub max_calls: usize,
    /// Tool calls in flight at once.
    pub parallel: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            memory: 64 * 1024 * 1024,
            stack: 512 * 1024,
            max_result: 64 * 1024,
            max_calls: 200,
            parallel: 8,
        }
    }
}

impl Limits {
    /// A requested timeout, clamped to something sensible.
    pub fn with_timeout_ms(mut self, requested: Option<u64>) -> Self {
        if let Some(ms) = requested {
            self.timeout = Duration::from_millis(ms).clamp(MIN_TIMEOUT, MAX_TIMEOUT);
        }
        self
    }
}

/// Stops a run from outside. Cheap to clone; all clones share one switch.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    flag: AtomicBool,
    wake: Notify,
}

impl Cancel {
    pub fn cancel(&self) {
        self.0.flag.store(true, Ordering::Relaxed);
        self.0.wake.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.flag.load(Ordering::Relaxed)
    }

    /// Resolves once cancelled; immediately if it already was.
    pub async fn cancelled(&self) {
        loop {
            let waiting = self.0.wake.notified();
            if self.is_cancelled() {
                return;
            }
            waiting.await;
        }
    }
}
