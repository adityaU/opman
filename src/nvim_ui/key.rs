//! Strongly typed keys and viewport dimensions for the Neovim UI pool.

use std::fmt;

/// The pool identity: one embedded Neovim per project/session pair.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionKey {
    pub project_idx: usize,
    pub session_id: String,
}

impl SessionKey {
    pub fn new(project_idx: usize, session_id: impl Into<String>) -> Self {
        Self {
            project_idx,
            session_id: session_id.into(),
        }
    }
}

/// Neovim rejects a zero-sized UI and unbounded values are not useful in a browser.
pub const MAX_UI_ROWS: u16 = 2_000;
pub const MAX_UI_COLS: u16 = 2_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiSize {
    rows: u16,
    cols: u16,
}

impl UiSize {
    pub fn new(rows: u16, cols: u16) -> Option<Self> {
        (rows > 0 && cols > 0 && rows <= MAX_UI_ROWS && cols <= MAX_UI_COLS)
            .then_some(Self { rows, cols })
    }

    pub fn rows(self) -> u16 {
        self.rows
    }

    pub fn cols(self) -> u16 {
        self.cols
    }
}

impl Default for UiSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

impl fmt::Display for UiSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.cols, self.rows)
    }
}

#[cfg(test)]
#[path = "key_tests.rs"]
mod key_tests;
