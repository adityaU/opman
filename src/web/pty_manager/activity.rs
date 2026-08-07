//! Whether a PTY is running a command or sitting at its prompt.
//!
//! The kernel already tracks this: a terminal has exactly one foreground
//! process group, and when a shell runs a command it hands that group over.
//! Comparing it against the program the PTY was spawned with is the whole
//! test — no polling of `/proc`, no parsing of output, no guessing from
//! keystrokes.

use serde::Serialize;

/// What a terminal is doing, as far as the kernel can tell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PtyActivity {
    /// The spawned program owns the terminal — a shell at its prompt, or a TUI
    /// with nothing shelled out. Also the answer when the foreground group is
    /// unknowable, because reporting work that may not exist is worse than
    /// reporting none.
    #[default]
    Idle,
    /// Another process group holds the terminal: a command is running.
    Running,
}

impl PtyActivity {
    /// Classify from the terminal's foreground process group and the pid of the
    /// program the PTY was spawned with.
    pub fn classify(foreground: Option<i32>, spawned: Option<u32>) -> Self {
        match (foreground, spawned) {
            (Some(group), Some(pid)) if group != pid as i32 => Self::Running,
            _ => Self::Idle,
        }
    }
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod activity_tests;
