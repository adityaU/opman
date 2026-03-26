//! Prompt history — CLI-style ArrowUp/ArrowDown recall of previous user messages.
//! Maintains a ring buffer of the last 100 user messages, a draft slot, and
//! a navigation index.  All state is local (no signals crossing component
//! boundaries) for minimal overhead.

use crate::types::core::Message;

/// Maximum number of user messages kept in the history ring.
const MAX_HISTORY: usize = 100;

/// Prompt history state — cheap to clone (inner Vecs are small).
#[derive(Clone, Debug)]
pub struct PromptHistory {
    /// Previous user messages, newest-last.
    entries: Vec<String>,
    /// Current index into `entries`.  `None` = not navigating (showing draft).
    index: Option<usize>,
    /// The draft the user was typing before entering history mode.
    draft: String,
}

impl PromptHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(MAX_HISTORY),
            index: None,
            draft: String::new(),
        }
    }

    // ── Population ──────────────────────────────────────────────────

    /// Rebuild the history buffer from the current message list.
    /// Only keeps messages with `role == "user"` that contain non-empty text.
    /// Called once on mount and whenever the message list meaningfully changes.
    pub fn rebuild(&mut self, messages: &[Message]) {
        self.entries.clear();
        for msg in messages {
            if msg.info.role != "user" {
                continue;
            }
            let text = Self::extract_text(msg);
            if text.is_empty() {
                continue;
            }
            self.entries.push(text);
        }
        // Keep only the most recent MAX_HISTORY entries.
        if self.entries.len() > MAX_HISTORY {
            let drain = self.entries.len() - MAX_HISTORY;
            self.entries.drain(..drain);
        }
        // Reset navigation whenever history is rebuilt.
        self.index = None;
    }

    /// Record a newly-sent user message (avoids a full rebuild).
    pub fn push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Deduplicate consecutive identical entries.
        if self.entries.last().map(|s| s.as_str()) == Some(text) {
            self.reset_nav();
            return;
        }
        self.entries.push(text.to_owned());
        if self.entries.len() > MAX_HISTORY {
            self.entries.remove(0);
        }
        self.reset_nav();
    }

    // ── Navigation ──────────────────────────────────────────────────

    /// Move backwards (older).  Returns the text to display, or `None`
    /// if already at the oldest entry.
    pub fn prev(&mut self, current_text: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let new_idx = match self.index {
            None => {
                // Entering history mode — save draft.
                self.draft = current_text.to_owned();
                self.entries.len() - 1
            }
            Some(0) => return None, // already at oldest
            Some(i) => i - 1,
        };
        self.index = Some(new_idx);
        Some(&self.entries[new_idx])
    }

    /// Move forwards (newer).  Returns the text to display.
    /// When moving past the newest entry, returns the saved draft and
    /// exits history mode.
    pub fn next(&mut self) -> Option<&str> {
        let idx = self.index?;
        if idx + 1 >= self.entries.len() {
            // Past newest → restore draft.
            self.index = None;
            Some(&self.draft)
        } else {
            let new_idx = idx + 1;
            self.index = Some(new_idx);
            Some(&self.entries[new_idx])
        }
    }

    /// Cancel history navigation without applying.
    pub fn reset_nav(&mut self) {
        self.index = None;
        self.draft.clear();
    }

    /// Whether we are currently navigating history.
    pub fn is_navigating(&self) -> bool {
        self.index.is_some()
    }

    // ── Search ──────────────────────────────────────────────────────

    /// Return entries matching `query` (case-insensitive), newest-first.
    /// Each item is `(original_index, &str)`.
    pub fn search<'a>(&'a self, query: &str) -> Vec<(usize, &'a str)> {
        if query.is_empty() {
            return Vec::new();
        }
        let lq = query.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, e)| e.to_lowercase().contains(&lq))
            .map(|(i, e)| (i, e.as_str()))
            .collect()
    }

    /// Get entry by index (used after search selection).
    pub fn get(&self, idx: usize) -> Option<&str> {
        self.entries.get(idx).map(|s| s.as_str())
    }

    // ── Helpers ─────────────────────────────────────────────────────

    /// Extract plain text from a user message's parts.
    fn extract_text(msg: &Message) -> String {
        let mut buf = String::new();
        for part in &msg.parts {
            if part.part_type == "text" {
                if let Some(ref t) = part.text {
                    if !buf.is_empty() {
                        buf.push('\n');
                    }
                    buf.push_str(t.trim());
                }
            }
        }
        buf
    }
}
