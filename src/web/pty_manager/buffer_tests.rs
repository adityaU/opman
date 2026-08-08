//! Generated unit tests for the raw PTY output ring buffer.

use super::*;
use std::sync::atomic::Ordering;

#[test]
fn new_buffer_is_empty_and_not_dirty() {
    let b = RawOutputBuffer::new();
    assert!(!b.dirty.load(Ordering::Acquire));
    assert!(b.drain_new().is_empty());
}

#[test]
fn push_sets_dirty_and_drain_returns_bytes() {
    let b = RawOutputBuffer::new();
    b.push(b"hello");
    assert!(b.dirty.load(Ordering::Acquire), "dirty flag set after push");
    let out = b.drain_new();
    assert_eq!(out, b"hello");
    // drain clears the dirty flag
    assert!(!b.dirty.load(Ordering::Acquire));
}

#[test]
fn drain_twice_returns_empty_second_time() {
    let b = RawOutputBuffer::new();
    b.push(b"abc");
    assert_eq!(b.drain_new(), b"abc");
    // Nothing new since last drain.
    assert!(b.drain_new().is_empty());
}

#[test]
fn multiple_pushes_accumulate_before_drain() {
    let b = RawOutputBuffer::new();
    b.push(b"foo");
    b.push(b"bar");
    b.push(b"baz");
    assert_eq!(b.drain_new(), b"foobarbaz");
}

#[test]
fn incremental_pushes_after_partial_drain() {
    let b = RawOutputBuffer::new();
    b.push(b"first");
    assert_eq!(b.drain_new(), b"first");
    b.push(b"second");
    assert_eq!(b.drain_new(), b"second");
}

#[test]
fn empty_push_produces_no_new_bytes() {
    let b = RawOutputBuffer::new();
    b.push(b"");
    // read_pos == buf.len() (both zero) -> drain returns empty
    assert!(b.drain_new().is_empty());
}

#[test]
fn eager_compaction_after_large_drain() {
    // Push more than COMPACT_THRESHOLD (256 KiB) so the eager compaction
    // branch in drain_new (read_pos >= COMPACT_THRESHOLD -> clear) fires.
    let big = vec![b'x'; 300 * 1024];
    let b = RawOutputBuffer::new();
    b.push(&big);
    let out = b.drain_new();
    assert_eq!(out.len(), big.len());
    // After eager compaction the buffer is cleared; a fresh push still works.
    b.push(b"tail");
    assert_eq!(b.drain_new(), b"tail");
}

#[test]
fn hard_cap_discards_oldest_unconsumed_data() {
    // Push more than MAX_BUFFER_BYTES (4 MiB) in a single call with read_pos == 0
    // so the hard-cap branch trims the buffer down to MAX_BUFFER_BYTES.
    let max = 4 * 1024 * 1024;
    let total = max + 1024 * 1024; // 5 MiB
    let data = vec![b'z'; total];
    let b = RawOutputBuffer::new();
    b.push(&data);
    let out = b.drain_new();
    // Buffer was capped to MAX_BUFFER_BYTES.
    assert_eq!(out.len(), max);
    // All retained bytes are the original filler value.
    assert!(out.iter().all(|&c| c == b'z'));
}

#[test]
fn snapshot_replays_bytes_already_drained() {
    let b = RawOutputBuffer::new();
    b.push(b"MARKER_42\n");
    assert_eq!(b.drain_new(), b"MARKER_42\n");
    // The live reader consumed it, but a re-attaching reader still gets it.
    assert_eq!(b.snapshot(), b"MARKER_42\n");
}

#[test]
fn snapshot_of_empty_buffer_is_empty() {
    let b = RawOutputBuffer::new();
    assert!(b.snapshot().is_empty());
}

#[test]
fn snapshot_seeks_reader_to_tip() {
    let b = RawOutputBuffer::new();
    b.push(b"history");
    assert_eq!(b.snapshot(), b"history");
    // Replay already covered those bytes; the drain loop must not repeat them.
    assert!(b.drain_new().is_empty());
    b.push(b"live");
    assert_eq!(b.drain_new(), b"live");
}

#[test]
fn snapshot_is_capped_at_the_retained_window() {
    let b = RawOutputBuffer::new();
    b.push(&vec![b'x'; RETAINED_SCROLLBACK * 2]);
    b.drain_new();
    assert_eq!(b.snapshot().len(), RETAINED_SCROLLBACK);
}

#[test]
fn compaction_keeps_the_retained_window() {
    let b = RawOutputBuffer::new();
    b.push(b"oldest\n");
    b.drain_new();
    // Enough consumed prefix to trip COMPACT_THRESHOLD.
    b.push(&vec![b'x'; COMPACT_THRESHOLD + RETAINED_SCROLLBACK]);
    b.drain_new();
    b.push(b"TAIL\n");
    assert_eq!(b.drain_new(), b"TAIL\n");

    let snap = b.snapshot();
    assert_eq!(snap.len(), RETAINED_SCROLLBACK);
    assert!(
        snap.ends_with(b"TAIL\n"),
        "newest output survives compaction"
    );
}

#[test]
fn snapshot_starts_on_a_line_boundary() {
    let b = RawOutputBuffer::new();
    // 64-byte lines, well past the retained window, so the cut lands mid-line.
    let line = b"L0123456789012345678901234567890123456789012345678901234567890\n";
    let reps = (RETAINED_SCROLLBACK * 2) / line.len();
    for _ in 0..reps {
        b.push(line);
    }
    b.drain_new();

    let snap = b.snapshot();
    assert!(snap.starts_with(b"L"), "replay opens on a fresh line");
    assert!(snap.len() < RETAINED_SCROLLBACK);
    assert!(snap.len() > RETAINED_SCROLLBACK - line.len());
}

#[test]
fn snapshot_without_newlines_is_not_trimmed_away() {
    let b = RawOutputBuffer::new();
    // A progress bar redrawing with \r and no \n at all.
    b.push(&vec![b'#'; RETAINED_SCROLLBACK * 2]);
    b.drain_new();
    // The line-alignment scan gives up rather than returning nothing.
    assert_eq!(b.snapshot().len(), RETAINED_SCROLLBACK);
}

#[test]
fn hard_cap_outranks_retention() {
    let b = RawOutputBuffer::new();
    b.push(&vec![b'a'; MAX_BUFFER_BYTES]);
    b.push(&vec![b'b'; 512 * 1024]);
    // The ceiling holds even though every byte is inside nothing droppable.
    assert!(b.snapshot().len() <= RETAINED_SCROLLBACK);
    assert_eq!(b.drain_new().len(), 0);
}

#[test]
fn clone_shares_underlying_buffer() {
    let b = RawOutputBuffer::new();
    let b2 = b.clone();
    b.push(b"shared");
    // The clone observes the same inner buffer.
    assert_eq!(b2.drain_new(), b"shared");
}
