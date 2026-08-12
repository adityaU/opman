//! Incremental, zero-copy MessagePack-RPC framing.

use std::ops::Range;

use super::scan::{read_array_len, read_str_slice, read_uint, skip_value, ScanError};

pub const DEFAULT_MAX_MESSAGE_LEN: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Request,
    Response,
    Notification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageSpan {
    pub kind: MessageKind,
    pub msgid: Option<u64>,
    /// UTF-8 method bytes, excluding the MessagePack string header.
    pub method_range: Range<usize>,
    /// Complete encoded params, or the result value for a response.
    pub params_range: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    Malformed,
    Oversized { length: usize, max: usize },
}

pub struct Framer {
    buffer: Vec<u8>,
    consumed: usize,
    max_message_len: usize,
}

impl Default for Framer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framer {
    pub fn new() -> Self {
        Self::with_max_message_len(DEFAULT_MAX_MESSAGE_LEN)
    }

    pub fn with_max_message_len(max_message_len: usize) -> Self {
        Self {
            buffer: Vec::new(),
            consumed: 0,
            max_message_len,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// A span indexes this slice until the next call to `next` or `push`.
    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    pub fn pending_len(&self) -> usize {
        self.buffer.len().saturating_sub(self.consumed)
    }

    pub fn next(&mut self) -> Result<Option<MessageSpan>, FrameError> {
        if self.consumed != 0 {
            self.buffer.drain(..self.consumed);
            self.consumed = 0;
        }
        if self.buffer.is_empty() {
            return Ok(None);
        }
        if let Some(length) = oversized_header(&self.buffer, self.max_message_len) {
            return Err(FrameError::Oversized {
                length,
                max: self.max_message_len,
            });
        }

        let mut end = 0;
        match skip_value(&self.buffer, &mut end) {
            Ok(()) => {}
            Err(ScanError::Incomplete) => return Ok(None),
            Err(ScanError::Malformed) => return Err(FrameError::Malformed),
        }
        if end > self.max_message_len {
            return Err(FrameError::Oversized {
                length: end,
                max: self.max_message_len,
            });
        }
        let span = parse_message(&self.buffer[..end])?;
        self.consumed = end;
        Ok(Some(span))
    }
}

fn oversized_header(buf: &[u8], max: usize) -> Option<usize> {
    let marker = *buf.first()?;
    let (width, extra) = match marker {
        0xc4 | 0xc7 | 0xd9 => (1, usize::from(marker == 0xc7)),
        0xc5 | 0xc8 | 0xda | 0xdc => (2, usize::from(matches!(marker, 0xc8))),
        0xc6 | 0xc9 | 0xdb | 0xdd => (4, usize::from(marker == 0xc9)),
        _ => return None,
    };
    let header_end = 1usize.checked_add(width)?;
    if buf.len() < header_end {
        return None;
    }
    let length = match width {
        1 => buf[1] as u64,
        2 => u16::from_be_bytes([buf[1], buf[2]]) as u64,
        4 => u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as u64,
        _ => return None,
    };
    let length = length.checked_add(extra as u64)?;
    if length <= max as u64 {
        return None;
    }
    let reported = match usize::try_from(length) {
        Ok(value) => value,
        Err(_) => usize::MAX,
    };
    Some(reported)
}

fn parse_message(buf: &[u8]) -> Result<MessageSpan, FrameError> {
    let mut cursor = 0;
    let fields = read_array_len(buf, &mut cursor).map_err(|_| FrameError::Malformed)?;
    let msg_type = read_uint(buf, &mut cursor).map_err(|_| FrameError::Malformed)?;
    match msg_type {
        0 if fields == 4 => {
            let msgid = read_uint(buf, &mut cursor).map_err(|_| FrameError::Malformed)?;
            let method_range = read_method_range(buf, &mut cursor)?;
            let params_start = cursor;
            skip_value(buf, &mut cursor).map_err(|_| FrameError::Malformed)?;
            Ok(MessageSpan {
                kind: MessageKind::Request,
                msgid: Some(msgid),
                method_range,
                params_range: params_start..cursor,
            })
        }
        1 if fields == 4 => {
            let msgid = read_uint(buf, &mut cursor).map_err(|_| FrameError::Malformed)?;
            skip_value(buf, &mut cursor).map_err(|_| FrameError::Malformed)?;
            let result_start = cursor;
            skip_value(buf, &mut cursor).map_err(|_| FrameError::Malformed)?;
            Ok(MessageSpan {
                kind: MessageKind::Response,
                msgid: Some(msgid),
                method_range: 0..0,
                params_range: result_start..cursor,
            })
        }
        2 if fields == 3 => {
            let method_range = read_method_range(buf, &mut cursor)?;
            let params_start = cursor;
            skip_value(buf, &mut cursor).map_err(|_| FrameError::Malformed)?;
            Ok(MessageSpan {
                kind: MessageKind::Notification,
                msgid: None,
                method_range,
                params_range: params_start..cursor,
            })
        }
        _ => Err(FrameError::Malformed),
    }
}

fn read_method_range(buf: &[u8], cursor: &mut usize) -> Result<Range<usize>, FrameError> {
    let start = *cursor;
    let method = read_str_slice(buf, cursor).map_err(|_| FrameError::Malformed)?;
    let payload_len = method.len();
    if payload_len > *cursor - start {
        return Err(FrameError::Malformed);
    }
    Ok((*cursor - payload_len)..*cursor)
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
