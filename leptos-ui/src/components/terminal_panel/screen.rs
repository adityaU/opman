//! Terminal screen state backed by vt100::Parser — data model, search, scrollback buffer.

use std::cell::RefCell;
use std::rc::Rc;

use super::render::render_screen_html;

/// Max lines to retain in the search buffer.
const SEARCH_BUFFER_MAX: usize = 600;

/// Shared terminal screen state backed by vt100::Parser.
#[derive(Clone)]
pub struct TermScreen {
    inner: Rc<RefCell<TermScreenInner>>,
}

struct TermScreenInner {
    parser: vt100::Parser,
    rows: u16,
    cols: u16,
    /// Full text buffer for search — includes scrollback + visible rows.
    search_lines: Vec<String>,
}

impl TermScreen {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            inner: Rc::new(RefCell::new(TermScreenInner {
                parser: vt100::Parser::new(rows, cols, 500),
                rows,
                cols,
                search_lines: Vec::new(),
            })),
        }
    }

    /// Process raw bytes from PTY output and update search buffer.
    pub fn process(&self, bytes: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        let old_visible = extract_visible_rows(&inner);
        inner.parser.process(bytes);
        update_search_buffer(&mut inner, &old_visible);
    }

    /// Resize the terminal screen.
    pub fn resize(&self, rows: u16, cols: u16) {
        let mut inner = self.inner.borrow_mut();
        inner.rows = rows;
        inner.cols = cols;
        inner.parser.screen_mut().set_size(rows, cols);
    }

    pub fn rows(&self) -> u16 {
        self.inner.borrow().rows
    }
    pub fn cols(&self) -> u16 {
        self.inner.borrow().cols
    }

    /// Extract plain text for all visible rows.
    pub fn extract_row_text(&self) -> Vec<String> {
        extract_visible_rows(&self.inner.borrow())
    }

    /// Find all matches of `query` (case-insensitive) across the full buffer.
    pub fn search(&self, query: &str) -> Vec<(usize, usize, usize)> {
        if query.is_empty() {
            return Vec::new();
        }
        search_lines(&self.inner.borrow().search_lines, query)
    }

    /// Find matches only within visible rows (for highlight rendering).
    pub fn search_visible(&self, query: &str) -> Vec<(usize, usize, usize)> {
        if query.is_empty() {
            return Vec::new();
        }
        let inner = self.inner.borrow();
        search_lines(&extract_visible_rows(&inner), query)
    }

    /// Render current screen to HTML line fragments.
    pub fn render_lines(&self, highlights: &[(usize, usize, usize, bool)]) -> Vec<String> {
        let inner = self.inner.borrow();
        let rows = inner.rows as usize;
        let cols = inner.cols as usize;
        render_screen_html(inner.parser.screen(), rows, cols, highlights)
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn extract_visible_rows(inner: &TermScreenInner) -> Vec<String> {
    let screen = inner.parser.screen();
    let rows = inner.rows as usize;
    let cols = inner.cols as usize;
    let mut result = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut line = String::with_capacity(cols);
        for col in 0..cols {
            let ch = screen
                .cell(row as u16, col as u16)
                .map(|c| c.contents())
                .unwrap_or_default();
            if ch.is_empty() {
                line.push(' ');
            } else {
                line.push_str(&ch);
            }
        }
        result.push(line);
    }
    result
}

/// Detect lines that scrolled off and accumulate into the search buffer.
fn update_search_buffer(inner: &mut TermScreenInner, old_visible: &[String]) {
    let new_visible = extract_visible_rows(inner);

    if old_visible.is_empty() {
        inner.search_lines = new_visible;
        return;
    }

    // Find how many old top lines scrolled off by checking alignment
    let old_top = &old_visible[0];
    let scrolled_off = if let Some(pos) = new_visible.iter().position(|l| l == old_top) {
        if pos > 0 {
            // Verify alignment: old[i] == new[pos+i]
            let aligned = (0..old_visible.len().min(new_visible.len() - pos))
                .all(|i| old_visible[i] == new_visible[pos + i]);
            if aligned {
                pos
            } else {
                0
            }
        } else {
            0
        }
    } else {
        // Old top is gone — assume all old visible lines scrolled off
        old_visible.len()
    };

    // Strip old visible tail from buffer, append scrolled-off lines, then new visible
    let buf_len = inner.search_lines.len();
    let visible_count = old_visible.len();
    if buf_len >= visible_count {
        inner.search_lines.truncate(buf_len - visible_count);
    }
    if scrolled_off > 0 {
        inner
            .search_lines
            .extend_from_slice(&old_visible[..scrolled_off]);
    }
    inner.search_lines.extend(new_visible);

    // Cap buffer size
    if inner.search_lines.len() > SEARCH_BUFFER_MAX {
        let drain = inner.search_lines.len() - SEARCH_BUFFER_MAX;
        inner.search_lines.drain(..drain);
    }
}

fn search_lines(lines: &[String], query: &str) -> Vec<(usize, usize, usize)> {
    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        let line_lower = line.to_lowercase();
        let mut start = 0;
        while let Some(pos) = line_lower[start..].find(&query_lower) {
            let col_start = start + pos;
            let col_end = col_start + query.len();
            matches.push((row, col_start, col_end));
            start = col_start + 1;
        }
    }
    matches
}
