//! Thread-safe raw output buffer for PTY output bytes.
//!
//! Accumulates raw PTY output and lets the SSE stream drain new bytes since the
//! last read. A trailing slice of the output is *retained* even after it has
//! been drained, so a browser that reloads and re-attaches to the still-running
//! PTY can repaint what is already on screen instead of coming back blank.
//!
//! Three sizes govern the buffer:
//!
//! - [`RETAINED_SCROLLBACK`] — the tail that compaction refuses to drop.
//! - [`COMPACT_THRESHOLD`]  — how much droppable prefix must pile up before it
//!   is worth a memmove.
//! - [`MAX_BUFFER_BYTES`]   — the hard memory ceiling, which outranks retention.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Hard limit on total buffer size. If the buffer exceeds this, older
/// data is discarded to keep memory bounded — retention does not override it.
const MAX_BUFFER_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

/// Trailing bytes kept for replay after they have been drained. Roughly a few
/// hundred lines of a wide terminal — enough to repaint a screen plus context.
const RETAINED_SCROLLBACK: usize = 128 * 1024; // 128 KiB

/// Minimum droppable prefix before compaction pays for its memmove.
const COMPACT_THRESHOLD: usize = 256 * 1024; // 256 KiB

/// How far into the retained window a replay may look for a line break before
/// giving up and starting mid-line. Bounds the scan, and keeps output that
/// simply has no newlines (a progress bar, a `cat` of binary) from replaying
/// as nothing at all.
const LINE_ALIGN_WINDOW: usize = 8 * 1024; // 8 KiB

/// Thread-safe buffer that accumulates raw PTY output bytes.
/// The SSE stream reads from `read_pos` and the reader thread appends.
#[derive(Clone, Debug)]
pub struct RawOutputBuffer {
    inner: Arc<Mutex<RawOutputInner>>,
    pub dirty: Arc<AtomicBool>,
}

#[derive(Debug)]
struct RawOutputInner {
    buf: Vec<u8>,
    /// How many bytes have been consumed by the SSE reader.
    read_pos: usize,
}

impl RawOutputInner {
    /// Offset at which the retained scrollback window begins.
    fn retained_start(&self) -> usize {
        self.buf.len().saturating_sub(RETAINED_SCROLLBACK)
    }

    /// Drop the leading bytes that are both consumed and outside the retained
    /// window, but only once enough have piled up to be worth the memmove.
    /// An over-cap buffer is trimmed regardless: the ceiling outranks retention.
    ///
    /// Allocation-free — `Vec::drain` moves the tail down in place.
    fn compact(&mut self) {
        let droppable = self.read_pos.min(self.retained_start());
        let over_cap = self.buf.len().saturating_sub(MAX_BUFFER_BYTES);
        let discard = match (over_cap, droppable >= COMPACT_THRESHOLD) {
            (0, false) => return,
            (0, true) => droppable,
            (over, _) => over.max(droppable),
        };
        self.buf.drain(..discard);
        self.read_pos = self.read_pos.saturating_sub(discard);
    }
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
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        inner.buf.extend_from_slice(data);
        inner.compact();
        self.dirty.store(true, Ordering::Release);
    }

    /// Read any new bytes since last call. Returns empty vec if nothing new.
    pub fn drain_new(&self) -> Vec<u8> {
        let Ok(mut inner) = self.inner.lock() else {
            return Vec::new();
        };
        if inner.read_pos >= inner.buf.len() {
            return Vec::new();
        }
        let data = inner.buf[inner.read_pos..].to_vec();
        inner.read_pos = inner.buf.len();
        self.dirty.store(false, Ordering::Release);
        inner.compact();
        data
    }

    /// Retained scrollback for a reader that is attaching to an already-running
    /// PTY, and seek that reader to the tip so the following [`Self::drain_new`]
    /// does not hand back the same bytes twice.
    ///
    /// When the window starts mid-stream it is trimmed to the next line break,
    /// so a replay never opens on half an escape sequence.
    pub fn snapshot(&self) -> Vec<u8> {
        let Ok(mut inner) = self.inner.lock() else {
            return Vec::new();
        };
        let start = match inner.retained_start() {
            0 => 0,
            cut => {
                let scan_end = inner.buf.len().min(cut + LINE_ALIGN_WINDOW);
                match inner.buf[cut..scan_end].iter().position(|&b| b == b'\n') {
                    Some(nl) => cut + nl + 1,
                    None => cut,
                }
            }
        };
        let data = inner.buf[start..].to_vec();
        inner.read_pos = inner.buf.len();
        self.dirty.store(false, Ordering::Release);
        data
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod buffer_tests;
