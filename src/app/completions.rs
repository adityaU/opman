use std::path::{Path, PathBuf};

use crate::app::App;

impl App {
    /// Expand `~` to the user's home directory in the input buffer.
    pub(super) fn expand_tilde(&self, input: &str) -> String {
        if input.starts_with('~') {
            let home = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .to_string_lossy()
                .to_string();
            format!("{}{}", home, &input[1..])
        } else {
            input.to_string()
        }
    }

    /// Scan the filesystem and update the completions list based on current input.
    pub fn update_completions(&mut self) {
        self.completions.clear();
        self.completion_selected = 0;

        let input = self.input_buffer.clone();
        if input.is_empty() {
            self.completions_visible = false;
            return;
        }

        let expanded = self.expand_tilde(&input);
        let path = Path::new(&expanded);

        let (parent, prefix) = if expanded.ends_with('/') {
            (path.to_path_buf(), String::new())
        } else {
            let parent = path.parent().unwrap_or(Path::new("/")).to_path_buf();
            let prefix = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            (parent, prefix)
        };

        let show_hidden = prefix.starts_with('.');

        let entries = match std::fs::read_dir(&parent) {
            Ok(rd) => rd,
            Err(_) => {
                self.completions_visible = false;
                return;
            }
        };

        let mut matches: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|entry| {
                let ft = entry.file_type().ok();
                let is_dir = ft.map(|f| f.is_dir()).unwrap_or(false);
                if !is_dir {
                    return false;
                }
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') && !show_hidden {
                    return false;
                }
                name_str.to_lowercase().starts_with(&prefix.to_lowercase())
            })
            .map(|entry| {
                let full = parent.join(entry.file_name());
                full.to_string_lossy().to_string()
            })
            .collect();

        matches.sort();

        // Convert back: if user typed ~, keep ~ prefix in completions
        if input.starts_with('~') {
            let home = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .to_string_lossy()
                .to_string();
            matches = matches
                .into_iter()
                .map(|m| {
                    if m.starts_with(&home) {
                        format!("~{}", &m[home.len()..])
                    } else {
                        m
                    }
                })
                .collect();
        }

        self.completions = matches;
        self.completions_visible = !self.completions.is_empty();
    }

    /// Apply the currently selected completion into the input buffer.
    pub fn apply_completion(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let idx = self.completion_selected.min(self.completions.len() - 1);
        let mut path = self.completions[idx].clone();
        if !path.ends_with('/') {
            path.push('/');
        }
        self.input_buffer = path;
        self.input_cursor = self.input_buffer.len();
        self.completions.clear();
        self.completion_selected = 0;
        self.completions_visible = false;
    }

    /// Complete the longest common prefix among all current completions.
    pub fn complete_common_prefix(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let first = &self.completions[0];
        let mut common_len = first.len();
        for c in &self.completions[1..] {
            common_len = common_len.min(
                first
                    .chars()
                    .zip(c.chars())
                    .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
                    .count(),
            );
        }
        let common: String = first.chars().take(common_len).collect();

        if common.len() > self.input_buffer.len() {
            self.input_buffer = common;
            self.input_cursor = self.input_buffer.len();
            // Re-scan to narrow down
            self.update_completions();
        }
    }

    /// Clear completion state.
    pub fn clear_completions(&mut self) {
        self.completions.clear();
        self.completion_selected = 0;
        self.completions_visible = false;
    }
}
