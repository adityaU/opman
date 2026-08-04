//! Generated tests for the pure PTY reader helper.
//!
//! The `spawn_*_pty` functions open a real PTY and launch external programs
//! ($SHELL, nvim, gitui, opencode, claude); they are not exercised here.
//! `read_raw_pty_output` is pure w.r.t. its `Read` source, so we drive it with
//! in-memory readers covering the EOF, data, and error paths.

use super::*;
use std::io::{self, Read};

/// A reader that yields `chunks` in order, then EOF (Ok(0)).
struct ChunkReader {
    chunks: Vec<Vec<u8>>,
    idx: usize,
}

impl Read for ChunkReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.idx >= self.chunks.len() {
            return Ok(0);
        }
        let chunk = &self.chunks[self.idx];
        let n = chunk.len().min(buf.len());
        buf[..n].copy_from_slice(&chunk[..n]);
        self.idx += 1;
        Ok(n)
    }
}

/// A reader that immediately errors.
struct ErrReader;
impl Read for ErrReader {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Other, "boom"))
    }
}

#[test]
fn reads_all_chunks_into_buffer_until_eof() {
    let output = RawOutputBuffer::new();
    let reader = ChunkReader {
        chunks: vec![b"hello ".to_vec(), b"world".to_vec()],
        idx: 0,
    };
    read_raw_pty_output(Box::new(reader), output.clone());
    assert_eq!(output.drain_new(), b"hello world");
}

#[test]
fn empty_reader_produces_no_output() {
    let output = RawOutputBuffer::new();
    let reader = ChunkReader {
        chunks: vec![],
        idx: 0,
    };
    read_raw_pty_output(Box::new(reader), output.clone());
    assert!(output.drain_new().is_empty());
}

#[test]
fn error_from_reader_stops_the_loop_gracefully() {
    let output = RawOutputBuffer::new();
    read_raw_pty_output(Box::new(ErrReader), output.clone());
    // Loop broke on the first Err without pushing anything.
    assert!(output.drain_new().is_empty());
}

#[test]
fn cursor_reader_reads_full_contents() {
    let output = RawOutputBuffer::new();
    let data = b"line1\nline2\n".to_vec();
    let cursor = io::Cursor::new(data.clone());
    read_raw_pty_output(Box::new(cursor), output.clone());
    assert_eq!(output.drain_new(), data);
}
