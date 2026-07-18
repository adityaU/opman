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
fn clone_shares_underlying_buffer() {
    let b = RawOutputBuffer::new();
    let b2 = b.clone();
    b.push(b"shared");
    // The clone observes the same inner buffer.
    assert_eq!(b2.drain_new(), b"shared");
}
