//! Thread-safe raw output buffer for PTY output bytes.
//!
//! Accumulates raw PTY output and lets the SSE stream drain new bytes
//! since the last read.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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
            // Compact if buffer grows too large and read_pos is past halfway
            let rp = inner.read_pos;
            if inner.buf.len() > 1_000_000 && rp > inner.buf.len() / 2 {
                inner.buf.drain(..rp);
                inner.read_pos = 0;
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
                return data;
            }
        }
        Vec::new()
    }
}
