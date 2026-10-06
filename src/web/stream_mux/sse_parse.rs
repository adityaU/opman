//! Incremental `text/event-stream` parser.
//!
//! Chunks are pushed as they arrive and events are pulled one at a time, so at most one
//! partial line and one partial event are ever held. Follows the WHATWG rules: `\n`,
//! `\r\n` and `\r` all end a line, `data:` lines join with `\n`, comments are skipped,
//! an event without data is not dispatched, and the last `id:` sticks.

use axum::body::Bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SseEvent {
    pub(crate) event: String,
    pub(crate) data: String,
    pub(crate) last_id: Option<String>,
}

#[derive(Default)]
pub(crate) struct SseParser {
    /// The unread rest of the last pushed chunk.
    pending: Bytes,
    /// A line split across chunks.
    line: Vec<u8>,
    /// The previous line ended in `\r`; a `\n` right after belongs to it.
    after_cr: bool,
    /// Past the optional byte-order mark at the start of the stream.
    started: bool,
    event: String,
    data: String,
    has_data: bool,
    last_id: Option<String>,
}

const BOM: &[u8] = "\u{feff}".as_bytes();

impl SseParser {
    /// Hand over the next chunk. Drain [`Self::next_event`] before pushing again.
    pub(crate) fn push(&mut self, chunk: Bytes) {
        if self.pending.is_empty() {
            self.pending = chunk;
            return;
        }
        let mut joined = Vec::with_capacity(self.pending.len() + chunk.len());
        joined.extend_from_slice(&self.pending);
        joined.extend_from_slice(&chunk);
        self.pending = Bytes::from(joined);
    }

    /// The next complete event in what has been pushed so far.
    pub(crate) fn next_event(&mut self) -> Option<SseEvent> {
        while let Some(line) = self.next_line() {
            if let Some(event) = self.process(&line) {
                return Some(event);
            }
        }
        None
    }

    fn next_line(&mut self) -> Option<Bytes> {
        if !self.started {
            if self.pending.len() < BOM.len() && BOM.starts_with(&self.pending) {
                return self.hold_rest();
            }
            if self.pending.starts_with(BOM) {
                let _ = self.pending.split_to(BOM.len());
            }
            self.started = true;
        }
        if self.after_cr && !self.pending.is_empty() {
            self.after_cr = false;
            if self.pending.first() == Some(&b'\n') {
                let _ = self.pending.split_to(1);
            }
        }
        let Some(end) = self
            .pending
            .iter()
            .position(|b| *b == b'\n' || *b == b'\r')
        else {
            return self.hold_rest();
        };
        self.after_cr = self.pending[end] == b'\r';
        let head = self.pending.split_to(end);
        let _ = self.pending.split_to(1);
        if self.line.is_empty() {
            return Some(head);
        }
        self.line.extend_from_slice(&head);
        Some(Bytes::from(std::mem::take(&mut self.line)))
    }

    /// Keep the unterminated tail for the next chunk.
    fn hold_rest(&mut self) -> Option<Bytes> {
        let rest = std::mem::take(&mut self.pending);
        if self.started {
            self.line.extend_from_slice(&rest);
        } else {
            self.pending = rest;
        }
        None
    }

    fn process(&mut self, line: &[u8]) -> Option<SseEvent> {
        if line.is_empty() {
            return self.dispatch();
        }
        if line.first() == Some(&b':') {
            return None;
        }
        let (field, value) = match line.iter().position(|b| *b == b':') {
            Some(colon) => {
                let value = &line[colon + 1..];
                (&line[..colon], value.strip_prefix(b" ").unwrap_or(value))
            }
            None => (line, &[][..]),
        };
        let value = String::from_utf8_lossy(value);
        match field {
            b"event" => self.event = value.into_owned(),
            b"data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(&value);
                self.has_data = true;
            }
            b"id" if !value.contains('\0') => self.last_id = Some(value.into_owned()),
            _ => {}
        }
        None
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        let event = std::mem::take(&mut self.event);
        if !std::mem::take(&mut self.has_data) {
            return None;
        }
        Some(SseEvent {
            event: if event.is_empty() {
                "message".to_owned()
            } else {
                event
            },
            data: std::mem::take(&mut self.data),
            last_id: self.last_id.clone(),
        })
    }
}
