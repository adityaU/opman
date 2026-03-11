/// Watcher modal types and state management.

/// Configuration for a session watcher that sends a continuation message
/// when the session goes idle for a configured duration.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WatcherConfig {
    pub session_id: String,
    pub project_idx: usize,
    pub idle_timeout_secs: u64,
    pub continuation_message: String,
    pub include_original: bool,
    /// The full text of the selected original user message (if any).
    pub original_message: Option<String>,
    /// Message sent after aborting a hung session.  When the session is busy but
    /// shows no MCP / PTY / message activity for `hang_timeout_secs`, the watcher
    /// will abort the session and send this message to retry.
    pub hang_message: String,
    /// Timeout in seconds for hang detection.  When the session has been busy
    /// with no activity signals (MCP, PTY output, SSE messages) for this many
    /// seconds, the overlay shows a "possibly hung" warning.  Default: 180 (3min).
    pub hang_timeout_secs: u64,
}

/// Which field is focused in the watcher modal right panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherField {
    SessionList,
    Message,
    IncludeOriginal,
    OriginalMessageList,
    TimeoutInput,
    HangMessage,
    HangTimeoutInput,
}

/// A session entry shown in the watcher modal left sidebar.
#[derive(Debug, Clone)]
pub struct WatcherSessionEntry {
    pub session_id: String,
    pub title: String,
    pub project_name: String,
    pub project_idx: usize,
    pub is_current: bool,
    pub is_active: bool,
    pub has_watcher: bool,
}

/// A user message from a session, for the "re-inject original" picker.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionMessage {
    pub role: String,
    pub text: String,
}

/// State for the watcher modal overlay.
pub struct WatcherModalState {
    /// Sessions displayed in the left sidebar.
    pub sessions: Vec<WatcherSessionEntry>,
    /// Currently selected session index in left sidebar.
    pub selected_session_idx: usize,
    /// Scroll offset for session list.
    pub session_scroll: usize,
    /// Which field is focused in the right panel.
    pub active_field: WatcherField,
    /// Continuation message lines (multi-line editor).
    pub message_lines: Vec<String>,
    pub message_cursor_row: usize,
    pub message_cursor_col: usize,
    /// Include original message toggle.
    pub include_original: bool,
    /// Messages from the selected session (fetched from API).
    pub session_messages: Vec<SessionMessage>,
    /// Selected original message index.
    pub selected_message_idx: usize,
    pub message_scroll: usize,
    /// Idle timeout seconds.
    pub idle_timeout_secs: u64,
    /// Timeout input buffer (for typing numbers).
    pub timeout_input: String,
    /// Hang detection message lines (multi-line editor).
    pub hang_message_lines: Vec<String>,
    pub hang_message_cursor_row: usize,
    pub hang_message_cursor_col: usize,
    /// Hang detection timeout seconds.
    pub hang_timeout_secs: u64,
    /// Hang timeout input buffer (for typing numbers).
    pub hang_timeout_input: String,
}

impl WatcherModalState {
    pub fn message_text(&self) -> String {
        self.message_lines.join("\n")
    }

    pub fn hang_message_text(&self) -> String {
        self.hang_message_lines.join("\n")
    }

    pub fn insert_char(&mut self, c: char) {
        self.message_lines[self.message_cursor_row].insert(self.message_cursor_col, c);
        self.message_cursor_col += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        let rest =
            self.message_lines[self.message_cursor_row][self.message_cursor_col..].to_string();
        self.message_lines[self.message_cursor_row].truncate(self.message_cursor_col);
        self.message_cursor_row += 1;
        self.message_lines.insert(self.message_cursor_row, rest);
        self.message_cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.message_cursor_col > 0 {
            let prev = self.message_lines[self.message_cursor_row][..self.message_cursor_col]
                .chars()
                .last()
                .unwrap_or(' ');
            self.message_cursor_col -= prev.len_utf8();
            self.message_lines[self.message_cursor_row].remove(self.message_cursor_col);
        } else if self.message_cursor_row > 0 {
            let line = self.message_lines.remove(self.message_cursor_row);
            self.message_cursor_row -= 1;
            self.message_cursor_col = self.message_lines[self.message_cursor_row].len();
            self.message_lines[self.message_cursor_row].push_str(&line);
        }
    }

    pub fn cursor_left(&mut self) {
        if self.message_cursor_col > 0 {
            let prev = self.message_lines[self.message_cursor_row][..self.message_cursor_col]
                .chars()
                .last()
                .unwrap_or(' ');
            self.message_cursor_col -= prev.len_utf8();
        } else if self.message_cursor_row > 0 {
            self.message_cursor_row -= 1;
            self.message_cursor_col = self.message_lines[self.message_cursor_row].len();
        }
    }

    pub fn cursor_right(&mut self) {
        let line_len = self.message_lines[self.message_cursor_row].len();
        if self.message_cursor_col < line_len {
            let next = self.message_lines[self.message_cursor_row][self.message_cursor_col..]
                .chars()
                .next()
                .unwrap_or(' ');
            self.message_cursor_col += next.len_utf8();
        } else if self.message_cursor_row + 1 < self.message_lines.len() {
            self.message_cursor_row += 1;
            self.message_cursor_col = 0;
        }
    }

    pub fn cursor_up(&mut self) {
        if self.message_cursor_row > 0 {
            self.message_cursor_row -= 1;
            self.message_cursor_col = self
                .message_cursor_col
                .min(self.message_lines[self.message_cursor_row].len());
        }
    }

    pub fn cursor_down(&mut self) {
        if self.message_cursor_row + 1 < self.message_lines.len() {
            self.message_cursor_row += 1;
            self.message_cursor_col = self
                .message_cursor_col
                .min(self.message_lines[self.message_cursor_row].len());
        }
    }

    // ── Hang message editor methods ──

    pub fn hang_insert_char(&mut self, c: char) {
        self.hang_message_lines[self.hang_message_cursor_row]
            .insert(self.hang_message_cursor_col, c);
        self.hang_message_cursor_col += c.len_utf8();
    }

    pub fn hang_insert_newline(&mut self) {
        let rest = self.hang_message_lines[self.hang_message_cursor_row]
            [self.hang_message_cursor_col..]
            .to_string();
        self.hang_message_lines[self.hang_message_cursor_row]
            .truncate(self.hang_message_cursor_col);
        self.hang_message_cursor_row += 1;
        self.hang_message_lines
            .insert(self.hang_message_cursor_row, rest);
        self.hang_message_cursor_col = 0;
    }

    pub fn hang_backspace(&mut self) {
        if self.hang_message_cursor_col > 0 {
            let prev = self.hang_message_lines[self.hang_message_cursor_row]
                [..self.hang_message_cursor_col]
                .chars()
                .last()
                .unwrap_or(' ');
            self.hang_message_cursor_col -= prev.len_utf8();
            self.hang_message_lines[self.hang_message_cursor_row]
                .remove(self.hang_message_cursor_col);
        } else if self.hang_message_cursor_row > 0 {
            let line = self.hang_message_lines.remove(self.hang_message_cursor_row);
            self.hang_message_cursor_row -= 1;
            self.hang_message_cursor_col =
                self.hang_message_lines[self.hang_message_cursor_row].len();
            self.hang_message_lines[self.hang_message_cursor_row].push_str(&line);
        }
    }

    pub fn hang_cursor_left(&mut self) {
        if self.hang_message_cursor_col > 0 {
            let prev = self.hang_message_lines[self.hang_message_cursor_row]
                [..self.hang_message_cursor_col]
                .chars()
                .last()
                .unwrap_or(' ');
            self.hang_message_cursor_col -= prev.len_utf8();
        } else if self.hang_message_cursor_row > 0 {
            self.hang_message_cursor_row -= 1;
            self.hang_message_cursor_col =
                self.hang_message_lines[self.hang_message_cursor_row].len();
        }
    }

    pub fn hang_cursor_right(&mut self) {
        let line_len = self.hang_message_lines[self.hang_message_cursor_row].len();
        if self.hang_message_cursor_col < line_len {
            let next = self.hang_message_lines[self.hang_message_cursor_row]
                [self.hang_message_cursor_col..]
                .chars()
                .next()
                .unwrap_or(' ');
            self.hang_message_cursor_col += next.len_utf8();
        } else if self.hang_message_cursor_row + 1 < self.hang_message_lines.len() {
            self.hang_message_cursor_row += 1;
            self.hang_message_cursor_col = 0;
        }
    }

    pub fn hang_cursor_up(&mut self) {
        if self.hang_message_cursor_row > 0 {
            self.hang_message_cursor_row -= 1;
            self.hang_message_cursor_col = self
                .hang_message_cursor_col
                .min(self.hang_message_lines[self.hang_message_cursor_row].len());
        }
    }

    pub fn hang_cursor_down(&mut self) {
        if self.hang_message_cursor_row + 1 < self.hang_message_lines.len() {
            self.hang_message_cursor_row += 1;
            self.hang_message_cursor_col = self
                .hang_message_cursor_col
                .min(self.hang_message_lines[self.hang_message_cursor_row].len());
        }
    }

    /// Cycle to the next field.
    pub fn next_field(&mut self) {
        self.active_field = match self.active_field {
            WatcherField::SessionList => WatcherField::Message,
            WatcherField::Message => WatcherField::IncludeOriginal,
            WatcherField::IncludeOriginal => {
                if self.include_original {
                    WatcherField::OriginalMessageList
                } else {
                    WatcherField::TimeoutInput
                }
            }
            WatcherField::OriginalMessageList => WatcherField::TimeoutInput,
            WatcherField::TimeoutInput => WatcherField::HangMessage,
            WatcherField::HangMessage => WatcherField::HangTimeoutInput,
            WatcherField::HangTimeoutInput => WatcherField::SessionList,
        };
    }

    /// Cycle to the previous field.
    pub fn prev_field(&mut self) {
        self.active_field = match self.active_field {
            WatcherField::SessionList => WatcherField::HangTimeoutInput,
            WatcherField::Message => WatcherField::SessionList,
            WatcherField::IncludeOriginal => WatcherField::Message,
            WatcherField::OriginalMessageList => WatcherField::IncludeOriginal,
            WatcherField::TimeoutInput => {
                if self.include_original {
                    WatcherField::OriginalMessageList
                } else {
                    WatcherField::IncludeOriginal
                }
            }
            WatcherField::HangMessage => WatcherField::TimeoutInput,
            WatcherField::HangTimeoutInput => WatcherField::HangMessage,
        };
    }
}
