//! Keeps the start and the end of a byte stream and counts what it dropped, so memory
//! stays bounded however much a command prints.

use crate::mcp_tools::limits::ByteLimit;

pub struct HeadTail {
    head: Vec<u8>,
    tail: Vec<u8>,
    head_cap: usize,
    tail_cap: usize,
    total: u64,
}

pub struct Captured {
    pub text: String,
    pub omitted: u64,
}

impl HeadTail {
    /// One third of the budget keeps the start; two thirds keep the end, where errors and
    /// results usually are.
    pub fn new(limit: ByteLimit) -> Self {
        let head_cap = limit.get() / 3;
        Self {
            head: Vec::new(),
            tail: Vec::new(),
            head_cap,
            tail_cap: limit.get() - head_cap,
            total: 0,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) {
        self.total += chunk.len() as u64;
        let room = self.head_cap.saturating_sub(self.head.len());
        let (to_head, rest) = chunk.split_at(room.min(chunk.len()));
        self.head.extend_from_slice(to_head);
        if rest.len() >= self.tail_cap {
            self.tail.clear();
            self.tail
                .extend_from_slice(&rest[rest.len() - self.tail_cap..]);
            return;
        }
        self.tail.extend_from_slice(rest);
        // Trim in batches so a long stream does not shift the buffer on every push.
        if self.tail.len() > self.tail_cap * 2 {
            let excess = self.tail.len() - self.tail_cap;
            self.tail.drain(..excess);
        }
    }

    pub fn finish(mut self) -> Captured {
        if self.tail.len() > self.tail_cap {
            let excess = self.tail.len() - self.tail_cap;
            self.tail.drain(..excess);
        }
        let omitted = self.total - (self.head.len() + self.tail.len()) as u64;
        if omitted == 0 {
            self.head.extend_from_slice(&self.tail);
            return Captured {
                text: lossy(self.head),
                omitted,
            };
        }
        let head = &self.head[..valid_prefix(&self.head)];
        let tail = &self.tail[char_start(&self.tail)..];
        let mut text = String::with_capacity(head.len() + tail.len() + 48);
        text.push_str(&String::from_utf8_lossy(head));
        text.push_str(&format!("\n[... {omitted} bytes omitted ...]\n"));
        text.push_str(&String::from_utf8_lossy(tail));
        Captured { text, omitted }
    }
}

fn lossy(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

/// Length of the longest prefix that does not end in half a character.
fn valid_prefix(bytes: &[u8]) -> usize {
    match std::str::from_utf8(bytes) {
        Ok(_) => bytes.len(),
        Err(error) if error.error_len().is_none() => error.valid_up_to(),
        Err(_) => bytes.len(),
    }
}

/// Offset of the first byte that starts a character.
fn char_start(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take(4)
        .position(|b| (b & 0b1100_0000) != 0b1000_0000)
        .unwrap_or(0)
}
