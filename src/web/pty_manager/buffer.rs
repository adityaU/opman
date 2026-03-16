//! Thread-safe raw output buffer for PTY output bytes.
//!
//! Accumulates raw PTY output and lets the SSE stream drain new bytes
//! since the last read. Includes hard cap to prevent unbounded growth.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Hard limit on total buffer size. If the buffer exceeds this, older
/// unconsumed data is discarded to keep memory bounded.
const MAX_BUFFER_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

/// Compaction threshold — compact when read_pos exceeds this fraction of buf.len().
const COMPACT_THRESHOLD: usize = 256 * 1024; // 256 KiB of consumed prefix

/// Thread-safe buffer that accumulates raw PTY output bytes.
/// The SSE stream reads from `read_pos` and the reader thread appends.
#[derive(Clone)]
pub struct RawOutputBuffer {
    inner: Arc<Mutex<RawOutputInner>>,
    pub dirty: Arc<AtomicBool>,
}

struct RawOutputInner {
    buf: Vec<u8>,
    /// How many bytes have been consumed by the SSE reader.
    read_pos: usize,
}

impl RawOutputBuffer {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RawOutputInner {
                buf: Vec::with_capacity(64 * 1024),
                read_pos: 0,
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Append raw bytes (called from PTY reader thread).
    pub(crate) fn push(&self, data: &[u8]) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.buf.extend_from_slice(data);

            // Compact consumed prefix when it exceeds COMPACT_THRESHOLD
            if inner.read_pos >= COMPACT_THRESHOLD {
                let rp = inner.read_pos;
                inner.buf.drain(..rp);
                inner.read_pos = 0;
            }

            // Hard cap: if buffer still exceeds MAX_BUFFER_BYTES, discard oldest
            // unconsumed data to stay within the limit.
            if inner.buf.len() > MAX_BUFFER_BYTES {
                let excess = inner.buf.len() - MAX_BUFFER_BYTES;
                let discard = excess.max(inner.read_pos);
                if inner.read_pos > discard {
                    inner.read_pos -= discard;
                } else {
                    inner.read_pos = 0;
                }
                inner.buf.drain(..discard);
            }

            self.dirty.store(true, Ordering::Release);
        }
    }

    /// Read any new bytes since last call. Returns empty slice if nothing new.
    pub fn drain_new(&self) -> Vec<u8> {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.read_pos < inner.buf.len() {
                let data = inner.buf[inner.read_pos..].to_vec();
                inner.read_pos = inner.buf.len();
                self.dirty.store(false, Ordering::Release);

                // Eagerly compact after drain if buffer is large
                if inner.read_pos >= COMPACT_THRESHOLD {
                    inner.buf.clear();
                    inner.read_pos = 0;
                }

                return data;
            }
        }
        Vec::new()
    }
}
