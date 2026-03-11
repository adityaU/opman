mod render;
mod render_results;
mod walker;

pub use render::FuzzyPicker;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};

/// State for the fuzzy directory picker (lives in App).
pub struct FuzzyPickerState {
    pub matcher: Nucleo<String>,
    pub query: String,
    pub prev_query: String,
    pub cursor_pos: usize,
    pub selected: u32,
    pub scroll_offset: u32,
    walk_complete_flag: Arc<AtomicBool>,
    /// Sidebar projects shown by default when query is empty.
    /// Each entry is (display_name, raw_path).
    pub existing_projects: Vec<(String, String)>,
}

impl std::fmt::Debug for FuzzyPickerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FuzzyPickerState")
            .field("query", &self.query)
            .field("selected", &self.selected)
            .field("walk_complete", &self.walk_complete())
            .finish()
    }
}

impl FuzzyPickerState {
    /// Create a new fuzzy picker and start scanning directories under `root`.
    #[allow(dead_code)]
    pub fn new(root: PathBuf) -> Self {
        Self::new_with_existing(root, Vec::new())
    }

    /// Create a fuzzy picker that includes existing project paths in results.
    pub fn new_with_existing(root: PathBuf, existing_projects: Vec<String>) -> Self {
        let matcher = Nucleo::new(
            Config::DEFAULT.match_paths(),
            Arc::new(|| {}),
            None, // use default thread count
            1,    // single match column
        );

        let injector = matcher.injector();
        let walk_done = Arc::new(AtomicBool::new(false));
        let walk_done_clone = Arc::clone(&walk_done);

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let home_str = home.to_string_lossy().to_string();
        let sidebar_projects: Vec<(String, String)> = existing_projects
            .iter()
            .map(|p| {
                let display = if p.starts_with(&home_str) {
                    format!("~{} ★", &p[home_str.len()..])
                } else {
                    format!("{} ★", p)
                };
                (display, p.clone())
            })
            .collect();

        std::thread::spawn(move || {
            walker::walk_directories(root, injector, existing_projects);
            walk_done_clone.store(true, Ordering::Release);
        });

        Self {
            matcher,
            query: String::new(),
            prev_query: String::new(),
            cursor_pos: 0,
            selected: 0,
            scroll_offset: 0,
            walk_complete_flag: walk_done,
            existing_projects: sidebar_projects,
        }
    }

    pub fn walk_complete(&self) -> bool {
        self.walk_complete_flag.load(Ordering::Acquire)
    }

    /// Tick the matcher and update pattern if query changed.
    /// Returns true if results changed.
    pub fn tick(&mut self) -> bool {
        // Update pattern if query changed
        if self.query != self.prev_query {
            let append = self.query.starts_with(&self.prev_query) && !self.prev_query.is_empty();
            self.matcher.pattern.reparse(
                0,
                &self.query,
                CaseMatching::Smart,
                Normalization::Smart,
                append,
            );
            self.prev_query = self.query.clone();
        }

        let status = self.matcher.tick(10);
        status.changed
    }

    /// Get the total number of matched items.
    pub fn matched_count(&self) -> u32 {
        self.matcher.snapshot().matched_item_count()
    }

    /// Get the total number of items (matched + unmatched).
    pub fn total_count(&self) -> u32 {
        self.matcher.snapshot().item_count()
    }

    /// Get the currently selected path, if any.
    pub fn selected_path(&self) -> Option<String> {
        if self.query.is_empty() && !self.existing_projects.is_empty() {
            let count = self.existing_projects.len() as u32;
            let idx = self.selected.min(count.saturating_sub(1));
            return self
                .existing_projects
                .get(idx as usize)
                .map(|(_, p)| p.clone());
        }

        let snapshot = self.matcher.snapshot();
        let count = snapshot.matched_item_count();
        if count == 0 {
            return None;
        }
        // Selection is from the bottom (fzf-style: 0 = bottom of list)
        let idx = self.selected.min(count.saturating_sub(1));
        let items: Vec<_> = snapshot.matched_items(0..count).collect();
        // Bottom-up: selected=0 means the top-scored item (first in matched_items)
        items.get(idx as usize).map(|item| item.data.clone())
    }

    /// Move selection up (towards less relevant items).
    pub fn move_up(&mut self) {
        let count = self.visible_count();
        if count > 0 && self.selected < count.saturating_sub(1) {
            self.selected += 1;
        }
    }

    /// Move selection down (towards more relevant items).
    pub fn move_down(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn visible_count(&self) -> u32 {
        if self.query.is_empty() && !self.existing_projects.is_empty() {
            self.existing_projects.len() as u32
        } else {
            self.matched_count()
        }
    }

    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.query.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Delete character before cursor.
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.query[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.remove(prev);
            self.cursor_pos = prev;
            self.selected = 0;
            self.scroll_offset = 0;
        }
    }

    /// Move cursor left.
    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.query[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right.
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
