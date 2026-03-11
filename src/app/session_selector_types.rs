use crate::app::SessionInfo;

/// A single entry in the cross-project session selector.
#[derive(Debug, Clone)]
pub struct SessionSelectorEntry {
    pub project_name: String,
    pub project_idx: usize,
    pub session: SessionInfo,
}

/// State for the cross-project session selector overlay.
pub struct SessionSelectorState {
    pub entries: Vec<SessionSelectorEntry>,
    pub query: String,
    pub cursor_pos: usize,
    pub selected: usize,
    pub scroll_offset: usize,
    pub filtered: Vec<usize>,
}

impl SessionSelectorState {
    /// Recompute filtered indices based on current query.
    pub fn update_filter(&mut self) {
        let query = self.query.to_lowercase();
        if query.is_empty() {
            self.filtered = (0..self.entries.len()).collect();
        } else {
            self.filtered = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    let haystack = format!("{} {}", e.project_name, e.session.title).to_lowercase();
                    haystack.contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
        self.scroll_offset = 0;
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else if !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() {
            if self.selected < self.filtered.len() - 1 {
                self.selected += 1;
            } else {
                self.selected = 0;
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.query.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.update_filter();
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.query[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.replace_range(prev..self.cursor_pos, "");
            self.cursor_pos = prev;
            self.update_filter();
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.query[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.query.len() {
            self.cursor_pos = self.query[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.query.len());
        }
    }
}

/// Connection status of a project's server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Error,
}
