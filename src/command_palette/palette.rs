use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};

use crate::config::KeyBindings;

use super::{build_commands, CommandAction, CommandEntry};

pub struct CommandPalette {
    pub query: String,
    pub cursor_pos: usize,
    pub selected: usize,
    pub scroll_offset: usize,
    matcher: Matcher,
    filtered: Vec<(usize, u32)>,
    commands: Vec<CommandEntry>,
}

impl CommandPalette {
    pub fn new(keys: &KeyBindings) -> Self {
        Self {
            query: String::new(),
            cursor_pos: 0,
            selected: 0,
            scroll_offset: 0,
            matcher: Matcher::new(Config::DEFAULT),
            filtered: Vec::new(),
            commands: build_commands(keys),
        }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.cursor_pos = 0;
        self.selected = 0;
        self.scroll_offset = 0;
        self.filtered.clear();
    }

    pub fn tick(&mut self) {
        self.filtered.clear();
        if self.query.is_empty() {
            for (i, _) in self.commands.iter().enumerate() {
                self.filtered.push((i, 0));
            }
        } else {
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Smart,
                Normalization::Smart,
                nucleo::pattern::AtomKind::Fuzzy,
            );
            for (i, cmd) in self.commands.iter().enumerate() {
                let haystack = format!("{} {}", cmd.name, cmd.shorthand);
                if let Some(score) = pattern.score(
                    nucleo::Utf32Str::Ascii(haystack.as_bytes()),
                    &mut self.matcher,
                ) {
                    self.filtered.push((i, score));
                }
            }
            self.filtered.sort_by(|a, b| b.1.cmp(&a.1));
        }
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn filtered_commands(&self) -> Vec<&CommandEntry> {
        self.filtered
            .iter()
            .map(|(i, _)| &self.commands[*i])
            .collect()
    }

    pub fn selected_action(&self) -> Option<CommandAction> {
        self.filtered
            .get(self.selected)
            .map(|(i, _)| self.commands[*i].action)
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered.len() - 1);
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.query.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.query[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.replace_range(prev..self.cursor_pos, "");
            self.cursor_pos = prev;
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.query[..self.cursor_pos]
                .char_indices()
                .last()
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
