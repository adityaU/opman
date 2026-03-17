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

/// Query params for `GET /api/git/diff?file=...&staged=...&repo=...`.
#[derive(Deserialize)]
pub struct GitDiffQuery {
    /// File path relative to repo root.
    pub file: Option<String>,
    /// If true, show staged (cached) diff. Default: false (unstaged).
    #[serde(default)]
    pub staged: bool,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
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

/// Query params for `GET /api/git/log?limit=...&repo=...`.
#[derive(Deserialize)]
pub struct GitLogQuery {
    /// Max number of commits to return (default 50).
    pub limit: Option<u32>,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/stage`.
#[derive(Deserialize)]
pub struct GitStageRequest {
    /// File paths to stage. Empty = stage all.
    pub files: Vec<String>,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/unstage`.
#[derive(Deserialize)]
pub struct GitUnstageRequest {
    /// File paths to unstage. Empty = unstage all.
    pub files: Vec<String>,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/commit`.
#[derive(Deserialize)]
pub struct GitCommitRequest {
    pub message: String,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
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
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
}

/// Query params for `GET /api/git/show?hash=...&repo=...`.
#[derive(Deserialize)]
pub struct GitShowQuery {
    /// Commit hash (full or short).
    pub hash: String,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
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
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
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

/// Query params for `GET /api/git/range-diff?base=<branch>&limit=<n>&repo=...`.
#[derive(Deserialize)]
pub struct GitRangeDiffQuery {
    /// Base branch to diff against (e.g. "main", "origin/main"). Default: "main".
    pub base: Option<String>,
    /// Max number of commits to include (default 50).
    pub limit: Option<u32>,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
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

// ── Multi-repo discovery types ──────────────────────────────────────

/// A discovered git repository within the workspace.
#[derive(Serialize, Clone)]
pub struct GitRepoEntry {
    /// Relative path from the project root to the repo root (e.g. "." or "packages/core").
    pub path: String,
    /// Human-friendly name (directory name or "root").
    pub name: String,
    /// Current branch.
    pub branch: String,
    /// Quick change counts.
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
}

/// Response for `GET /api/git/repos`.
#[derive(Serialize)]
pub struct GitReposResponse {
    /// All git repositories discovered under the workspace root.
    pub repos: Vec<GitRepoEntry>,
}

/// Query param used by repo-scoped git endpoints: `?repo=<relative_path>`.
/// When omitted, falls back to the project root (existing single-repo behaviour).
#[derive(Deserialize, Default)]
pub struct GitRepoScope {
    /// Relative path to the repo root from the project dir (default: ".").
    #[serde(default)]
    pub repo: String,
}

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

/// Request body for `POST /api/git/stash`.
#[derive(Deserialize)]
pub struct GitStashRequest {
    /// Action: "push" (default), "pop", "list", "drop".
    #[serde(default = "stash_action_default")]
    pub action: String,
    /// Optional stash message (for push).
    #[serde(default)]
    pub message: String,
    /// Optional stash index (for pop/drop, e.g. "stash@{0}").
    #[serde(default)]
    pub stash_ref: String,
    /// Repo path relative to project root (default: ".").
    #[serde(default)]
    pub repo: String,
}

fn stash_action_default() -> String {
    "push".to_string()
}

/// A single stash entry.
#[derive(Serialize)]
pub struct GitStashEntry {
    pub index: usize,
    pub reference: String,
    pub message: String,
}

/// Response for `POST /api/git/stash`.
#[derive(Serialize)]
pub struct GitStashResponse {
    pub success: bool,
    pub output: String,
    /// Populated when action is "list".
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
