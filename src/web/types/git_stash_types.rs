//! Pull, stash and gitignore types.

use serde::{Deserialize, Serialize};

use super::git_ops::GitActionResponse;

// ── Pull / Stash / Gitignore types ─────────────────────────────────

/// Request body for `POST /api/git/pull`.
#[derive(Deserialize)]
pub struct GitPullRequest {
    /// Optional remote name (default: "origin").
    #[serde(default)]
    pub remote: String,
    /// Optional branch to pull (default: current branch).
    #[serde(default)]
    pub branch: String,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
}

/// Response for `POST /api/git/pull`.
#[derive(Serialize)]
pub struct GitPullResponse {
    pub success: bool,
    pub output: String,
}

/// What a stash request should do.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GitStashAction {
    /// Save the working tree, including untracked files.
    #[default]
    Push,
    /// Restore an entry and remove it from the list.
    Pop,
    /// Restore an entry, keeping it in the list.
    Apply,
    /// Discard an entry without restoring it. Destructive.
    Drop,
    /// Read the list.
    List,
}

impl GitStashAction {
    /// The git subcommand, which is also the proof each action is handled.
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Pop => "pop",
            Self::Apply => "apply",
            Self::Drop => "drop",
            Self::List => "list",
        }
    }
}

/// Request body for `POST /api/git/stash`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStashRequest {
    #[serde(default)]
    pub action: GitStashAction,
    /// Label for a saved stash.
    #[serde(default)]
    pub message: Option<String>,
    /// Entry to act on, such as `stash@{0}`. Absent means the most recent.
    #[serde(default)]
    pub stash_ref: Option<String>,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
}

/// A single stash entry.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStashEntry {
    pub index: usize,
    /// The ref that addresses this entry, such as `stash@{0}`.
    pub reference: String,
    pub message: String,
    /// Relative age, in git's own wording.
    pub age: String,
    pub hash: String,
}

/// Response for `POST /api/git/stash`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStashResponse {
    #[serde(flatten)]
    pub action: GitActionResponse,
    /// Populated only by the list action.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<GitStashEntry>,
}

/// Request body for `POST /api/git/gitignore`.
#[derive(Deserialize)]
pub struct GitIgnoreRequest {
    /// Action: "add" or "list".
    #[serde(default = "gitignore_action_default")]
    pub action: String,
    /// Patterns to add (for "add" action).
    #[serde(default)]
    pub patterns: Vec<String>,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
}

fn gitignore_action_default() -> String {
    "list".to_string()
}

/// Response for `POST /api/git/gitignore`.
#[derive(Serialize)]
pub struct GitIgnoreResponse {
    pub success: bool,
    /// Current .gitignore contents.
    pub content: String,
}

#[cfg(test)]
#[path = "git_stash_types_tests.rs"]
mod git_stash_types_tests;
