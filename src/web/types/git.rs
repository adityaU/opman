//! Git API types.

use serde::{Deserialize, Serialize};

/// A single file entry in `git status` output.
#[derive(Serialize, Clone)]
pub struct GitFileEntry {
    pub path: String,
    /// Status code: "M" (modified), "A" (added), "D" (deleted), "R" (renamed),
    /// "?" (untracked), "U" (unmerged), etc.
    pub status: String,
}

/// Response for `GET /api/git/status`.
#[derive(Serialize)]
pub struct GitStatusResponse {
    pub branch: String,
    pub staged: Vec<GitFileEntry>,
    pub unstaged: Vec<GitFileEntry>,
    pub untracked: Vec<GitFileEntry>,
}

/// Response for `GET /api/git/diff`.
#[derive(Serialize)]
pub struct GitDiffResponse {
    pub diff: String,
}

/// Query params for `GET /api/git/diff?file=...&staged=...`.
#[derive(Deserialize)]
pub struct GitDiffQuery {
    /// File path relative to repo root.
    pub file: Option<String>,
    /// If true, show staged (cached) diff. Default: false (unstaged).
    #[serde(default)]
    pub staged: bool,
}

/// A single commit entry.
#[derive(Serialize)]
pub struct GitLogEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

/// Response for `GET /api/git/log`.
#[derive(Serialize)]
pub struct GitLogResponse {
    pub commits: Vec<GitLogEntry>,
}

/// Query params for `GET /api/git/log?limit=...`.
#[derive(Deserialize)]
pub struct GitLogQuery {
    /// Max number of commits to return (default 50).
    pub limit: Option<u32>,
}

/// Request body for `POST /api/git/stage`.
#[derive(Deserialize)]
pub struct GitStageRequest {
    /// File paths to stage. Empty = stage all.
    pub files: Vec<String>,
}

/// Request body for `POST /api/git/unstage`.
#[derive(Deserialize)]
pub struct GitUnstageRequest {
    /// File paths to unstage. Empty = unstage all.
    pub files: Vec<String>,
}

/// Request body for `POST /api/git/commit`.
#[derive(Deserialize)]
pub struct GitCommitRequest {
    pub message: String,
}

/// Response for `POST /api/git/commit`.
#[derive(Serialize)]
pub struct GitCommitResponse {
    pub hash: String,
    pub message: String,
}

/// Request body for `POST /api/git/discard`.
#[derive(Deserialize)]
pub struct GitDiscardRequest {
    /// File paths to discard changes for.
    pub files: Vec<String>,
}

/// Query params for `GET /api/git/show?hash=...`.
#[derive(Deserialize)]
pub struct GitShowQuery {
    /// Commit hash (full or short).
    pub hash: String,
}

/// Response for `GET /api/git/show`.
#[derive(Serialize)]
pub struct GitShowResponse {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
    pub diff: String,
    /// List of files changed in this commit.
    pub files: Vec<GitShowFile>,
}

/// A file changed in a commit.
#[derive(Serialize)]
pub struct GitShowFile {
    pub path: String,
    pub status: String,
}

/// Response for `GET /api/git/branches`.
#[derive(Serialize)]
pub struct GitBranchesResponse {
    /// The current (checked-out) branch name.
    pub current: String,
    /// Local branch names.
    pub local: Vec<String>,
    /// Remote branch names (e.g. "origin/main").
    pub remote: Vec<String>,
}

/// Request body for `POST /api/git/checkout`.
#[derive(Deserialize)]
pub struct GitCheckoutRequest {
    /// Branch name to switch to.
    pub branch: String,
}

/// Response for `POST /api/git/checkout`.
#[derive(Serialize)]
pub struct GitCheckoutResponse {
    /// The branch that was switched to.
    pub branch: String,
    /// `true` if checkout succeeded.
    pub success: bool,
    /// Optional message (e.g. stderr output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Query params for `GET /api/git/range-diff?base=<branch>&limit=<n>`.
#[derive(Deserialize)]
pub struct GitRangeDiffQuery {
    /// Base branch to diff against (e.g. "main", "origin/main"). Default: "main".
    pub base: Option<String>,
    /// Max number of commits to include (default 50).
    pub limit: Option<u32>,
}

/// Response for `GET /api/git/range-diff`.
#[derive(Serialize)]
pub struct GitRangeDiffResponse {
    /// Current branch name.
    pub branch: String,
    /// Base branch diffed against.
    pub base: String,
    /// Commits in the range (current branch only).
    pub commits: Vec<GitLogEntry>,
    /// Cumulative diff (all changes between base..HEAD).
    pub diff: String,
    /// Number of files changed.
    pub files_changed: usize,
}

/// Response for `GET /api/git/context-summary`.
#[derive(Serialize)]
pub struct GitContextSummaryResponse {
    /// Current branch name.
    pub branch: String,
    /// Recent commits on the current branch (up to 5).
    pub recent_commits: Vec<GitLogEntry>,
    /// Number of staged files.
    pub staged_count: usize,
    /// Number of unstaged (modified) files.
    pub unstaged_count: usize,
    /// Number of untracked files.
    pub untracked_count: usize,
    /// Short summary suitable for AI context injection.
    pub summary: String,
}
