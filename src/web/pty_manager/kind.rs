//! What a web PTY runs, and the request to start one.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What program a web PTY runs.
///
/// An enum rather than the wire string it arrives as: the string used to be
/// re-matched at every layer, so a kind the spawner did not know reached it
/// three calls deep instead of being refused at the edge.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PtyKind {
    Shell,
    Neovim,
    Git,
    Opencode,
    ClaudeAttach,
}

impl PtyKind {
    /// How the kind reads in a list the user is looking at.
    pub fn label(self) -> &'static str {
        match self {
            Self::Shell => "Shell",
            Self::Neovim => "Neovim",
            Self::Git => "Git",
            Self::Opencode => "OpenCode",
            Self::ClaudeAttach => "Claude",
        }
    }
}

/// What to run, carrying exactly the arguments that one kind needs.
///
/// The per-kind data lives in its own arm so the combinations that used to be
/// possible — a `claude-attach` with no agent to attach to, a shell holding a
/// session id — cannot be built at all.
pub enum PtyProgram {
    Shell,
    Neovim,
    Git,
    /// `None` starts a fresh conversation rather than resuming one.
    Opencode { session_id: Option<String> },
    /// The claude background agent's short id.
    ClaudeAttach { short_id: String },
}

impl PtyProgram {
    pub fn kind(&self) -> PtyKind {
        match self {
            Self::Shell => PtyKind::Shell,
            Self::Neovim => PtyKind::Neovim,
            Self::Git => PtyKind::Git,
            Self::Opencode { .. } => PtyKind::Opencode,
            Self::ClaudeAttach { .. } => PtyKind::ClaudeAttach,
        }
    }
}

/// Everything needed to start one PTY and to describe it afterwards.
///
/// `project` is the spawn request's own rather than whichever project the shell
/// happens to have focused: a terminal in a pane opened on one repo must not
/// start in another one just because the sidebar moved.
pub struct SpawnSpec {
    pub id: String,
    pub program: PtyProgram,
    pub project: PathBuf,
    /// What the shell is called in the picker, or `None` to have the manager
    /// number it within its project. Numbering belongs to whoever holds the
    /// whole map — a caller that counts its own shells cannot see the ones
    /// another pane started.
    pub label: Option<String>,
    pub rows: u16,
    pub cols: u16,
}

#[cfg(test)]
#[path = "kind_tests.rs"]
mod kind_tests;
