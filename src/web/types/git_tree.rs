//! Types for worktrees, history integration and the in-progress operation state.

use serde::{Deserialize, Serialize};

/// One entry from `git worktree list`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeEntry {
    /// Absolute path on disk.
    pub path: String,
    /// Path relative to the project root, which is what the panel shows and
    /// what a later request passes back as its `repo` scope. Absent when the
    /// worktree lives outside the project tree and so cannot be scoped to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative: Option<String>,
    /// Branch checked out here; absent when this worktree is detached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub head: String,
    /// The worktree containing the repository itself.
    pub main: bool,
    /// True when this is the worktree the request was scoped to.
    pub current: bool,
    pub locked: bool,
    /// Set when git reports the worktree can be pruned, with its reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prunable: Option<String>,
}

/// Response for `GET /api/git/worktrees`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreesResponse {
    pub worktrees: Vec<GitWorktreeEntry>,
}

/// Request body for `POST /api/git/worktree/add`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeAddRequest {
    /// Destination, relative to the project root.
    pub path: String,
    /// Branch to check out there.
    pub branch: String,
    /// Create `branch` rather than checking out an existing one.
    #[serde(default)]
    pub create: bool,
    /// Commit-ish the new branch starts from. Only read when `create`.
    #[serde(default)]
    pub start_point: Option<String>,
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/worktree/remove`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeRemoveRequest {
    pub path: String,
    /// Remove even with uncommitted changes present. Destructive.
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub repo: String,
}

/// Which multi-step operation the repository is in the middle of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitOperationKind {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
}

/// Response for `GET /api/git/operation` — what is in flight, and what is stuck.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOperationResponse {
    /// Absent when the repository is in a clean state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<GitOperationKind>,
    /// Paths with unresolved conflict markers.
    pub conflicted: Vec<String>,
    /// For a rebase: which step of how many.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    /// Ref being replayed onto, when git records one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onto: Option<String>,
}

/// How to finish or abandon an in-flight operation.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GitOperationAction {
    Continue,
    Abort,
    Skip,
}

/// Request body for `POST /api/git/operation`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOperationRequest {
    pub action: GitOperationAction,
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/merge`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitMergeRequest {
    /// Branch merged into the current one.
    pub branch: String,
    /// Record a merge commit even when a fast-forward was possible.
    #[serde(default)]
    pub no_ff: bool,
    /// Stop before committing so the result can be reviewed.
    #[serde(default)]
    pub no_commit: bool,
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/rebase`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRebaseRequest {
    /// Branch the current one is replayed onto.
    pub onto: String,
    #[serde(default)]
    pub repo: String,
}

/// How far a reset moves the index and working tree.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GitResetMode {
    /// Move the branch only; index and working tree keep their contents.
    Soft,
    /// Move the branch and reset the index; the working tree is untouched.
    Mixed,
    /// Move everything. Uncommitted work is destroyed.
    Hard,
}

impl GitResetMode {
    pub const fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
        }
    }
}

/// Request body for `POST /api/git/reset`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitResetRequest {
    /// Commit to move to.
    pub target: String,
    pub mode: GitResetMode,
    #[serde(default)]
    pub repo: String,
}

/// Request body for the commit-replay endpoints `revert` and `cherry-pick`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitReplayRequest {
    pub hash: String,
    /// Apply the change without committing it.
    #[serde(default)]
    pub no_commit: bool,
    #[serde(default)]
    pub repo: String,
}

/// One tag.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTagEntry {
    pub name: String,
    pub hash: String,
    /// Annotation subject for an annotated tag, else the commit subject.
    pub subject: String,
    pub date: String,
}

/// Response for `GET /api/git/tags`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTagsResponse {
    pub tags: Vec<GitTagEntry>,
}

/// Request body for `POST /api/git/tag`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTagRequest {
    pub name: String,
    /// Annotation text. An annotated tag is created when this is present.
    #[serde(default)]
    pub message: Option<String>,
    /// Commit to tag. Defaults to HEAD.
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/tag/delete`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTagDeleteRequest {
    pub name: String,
    #[serde(default)]
    pub repo: String,
}

/// One line of `git blame` output.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBlameLine {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub summary: String,
    pub line: u32,
    pub content: String,
}

/// Response for `GET /api/git/blame`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBlameResponse {
    pub lines: Vec<GitBlameLine>,
}

/// Query params for `GET /api/git/blame`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBlameQuery {
    pub file: String,
    #[serde(default)]
    pub repo: String,
}

#[cfg(test)]
#[path = "git_tree_tests.rs"]
mod git_tree_tests;
