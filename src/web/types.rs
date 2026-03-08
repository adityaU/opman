//! Serializable types for the web API.
//!
//! These mirror the internal App/Session/PTY types but are decoupled for
//! independent evolution and to avoid leaking internal details.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::mcp::NvimSocketRegistry;

use super::pty_manager::WebPtyHandle;
use super::web_state::WebStateHandle;

// ── Broadcast events from web state → web clients ───────────────────

/// Events pushed to connected web clients via SSE.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum WebEvent {
    StateChanged,
    SessionBusy {
        session_id: String,
    },
    SessionIdle {
        session_id: String,
    },
    StatsUpdated(WebSessionStats),
    ThemeChanged(WebThemeColors),
    /// Watcher status changed (created, deleted, countdown, triggered).
    WatcherStatusChanged(WatcherStatusEvent),
    /// MCP: AI agent opened a file in the editor.
    McpEditorOpen {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
    },
    /// MCP: AI agent navigated to a line in the editor.
    McpEditorNavigate {
        line: u32,
    },
    /// MCP: AI agent focused a terminal tab.
    McpTerminalFocus {
        id: String,
    },
    /// MCP: AI agent activity indicator (tool being invoked).
    McpAgentActivity {
        tool: String,
        active: bool,
    },
    /// Session activity event (file edit, tool call, terminal command, permission request).
    ActivityEvent(ActivityEventPayload),
    /// Presence changed — a client connected/disconnected or focused a session.
    PresenceChanged(PresenceSnapshot),
    /// Initial value — never sent to clients.
    Noop,
}

// ── Theme colors (hex strings for the web frontend) ─────────────────

/// Serializable theme colors — 15 hex string fields matching the TUI's `ThemeColors` struct.
/// Sent to web clients via `/api/theme` and SSE `theme_changed` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebThemeColors {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub background: String,
    pub background_panel: String,
    pub background_element: String,
    pub text: String,
    pub text_muted: String,
    pub border: String,
    pub border_active: String,
    pub border_subtle: String,
    pub error: String,
    pub warning: String,
    pub success: String,
    pub info: String,
}

impl WebThemeColors {
    /// Convert a TUI `ThemeColors` into serializable hex strings.
    pub fn from_theme(theme: &crate::theme::ThemeColors) -> Self {
        Self {
            primary: color_to_hex(theme.primary),
            secondary: color_to_hex(theme.secondary),
            accent: color_to_hex(theme.accent),
            background: color_to_hex(theme.background),
            background_panel: color_to_hex(theme.background_panel),
            background_element: color_to_hex(theme.background_element),
            text: color_to_hex(theme.text),
            text_muted: color_to_hex(theme.text_muted),
            border: color_to_hex(theme.border),
            border_active: color_to_hex(theme.border_active),
            border_subtle: color_to_hex(theme.border_subtle),
            error: color_to_hex(theme.error),
            warning: color_to_hex(theme.warning),
            success: color_to_hex(theme.success),
            info: color_to_hex(theme.info),
        }
    }
}

/// Convert a ratatui `Color` to a CSS hex string (e.g. `#fab283`).
fn color_to_hex(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        _ => "#808080".to_string(), // Fallback for non-RGB colors
    }
}

// ── App state snapshot ──────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct WebAppState {
    pub projects: Vec<WebProjectInfo>,
    pub active_project: usize,
    pub panels: WebPanelVisibility,
    pub focused: String,
}

#[derive(Serialize, Clone)]
pub struct WebProjectInfo {
    pub name: String,
    pub path: String,
    pub index: usize,
    pub active_session: Option<String>,
    pub sessions: Vec<WebSessionInfo>,
    pub git_branch: String,
    pub busy_sessions: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct WebSessionInfo {
    pub id: String,
    pub title: String,
    #[serde(rename = "parentID")]
    pub parent_id: String,
    pub directory: String,
    pub time: WebSessionTime,
}

#[derive(Serialize, Clone)]
pub struct WebSessionTime {
    pub created: u64,
    pub updated: u64,
}

#[derive(Serialize, Clone)]
pub struct WebPanelVisibility {
    pub sidebar: bool,
    pub terminal_pane: bool,
    pub neovim_pane: bool,
    pub integrated_terminal: bool,
    pub git_panel: bool,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct WebSessionStats {
    pub cost: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

// ── Context Window types ────────────────────────────────────────────

/// Response for `GET /api/context-window`.
///
/// Provides a breakdown of context window usage for the active session,
/// including total limit, used tokens by category, and per-item estimates.
#[derive(Serialize, Clone, Debug)]
pub struct ContextWindowResponse {
    /// Maximum context window size in tokens for the active model.
    pub context_limit: u64,
    /// Total tokens currently used across all categories.
    pub total_used: u64,
    /// Usage percentage (0–100).
    pub usage_pct: f64,
    /// Breakdown by category.
    pub categories: Vec<ContextCategory>,
    /// Estimated messages remaining at current rate.
    pub estimated_messages_remaining: Option<u64>,
}

/// A single category of context window usage.
#[derive(Serialize, Clone, Debug)]
pub struct ContextCategory {
    /// Category name: "system", "messages", "tool_results", "files", "cache"
    pub name: String,
    /// Human-readable label.
    pub label: String,
    /// Tokens consumed by this category.
    pub tokens: u64,
    /// Percentage of total context window.
    pub pct: f64,
    /// Color hint for the frontend: "blue", "green", "orange", "purple", "gray"
    pub color: String,
    /// Individual items within this category (if available).
    pub items: Vec<ContextItem>,
}

/// An individual item contributing to context usage.
#[derive(Serialize, Clone, Debug)]
pub struct ContextItem {
    /// Item description (e.g. message preview, file path, tool name).
    pub label: String,
    /// Estimated tokens for this item.
    pub tokens: u64,
}

// ── Request body types ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Deserialize)]
pub struct SwitchProjectRequest {
    pub index: usize,
}

#[derive(Deserialize)]
pub struct SelectSessionRequest {
    pub project_idx: usize,
    pub session_id: String,
}

#[derive(Deserialize)]
pub struct NewSessionRequest {
    pub project_idx: usize,
}

/// Response from creating a new session.
#[derive(Serialize)]
pub struct NewSessionResponse {
    pub session_id: String,
}

/// Request to add a new project.
#[derive(Deserialize)]
pub struct AddProjectRequest {
    /// Absolute path to the project directory.
    pub path: String,
    /// Optional display name. If not provided, the directory name is used.
    #[serde(default)]
    pub name: Option<String>,
}

/// Response after successfully adding a project.
#[derive(Serialize)]
pub struct AddProjectResponse {
    /// The index of the newly added project.
    pub index: usize,
    /// The resolved project name.
    pub name: String,
}

/// Request to remove a project.
#[derive(Deserialize)]
pub struct RemoveProjectRequest {
    pub index: usize,
}

#[derive(Deserialize)]
pub struct TogglePanelRequest {
    pub panel: String,
}

#[derive(Deserialize)]
pub struct FocusPanelRequest {
    pub panel: String,
}

/// Request to spawn a web PTY.
#[derive(Deserialize)]
pub struct SpawnPtyRequest {
    /// PTY type: "shell", "neovim", "git", or "opencode"
    pub kind: String,
    /// Unique ID for this PTY instance (client-generated)
    pub id: String,
    pub rows: Option<u16>,
    pub cols: Option<u16>,
    /// Optional session ID (only used for "opencode" kind)
    pub session_id: Option<String>,
}

/// Request to write to a web PTY.
#[derive(Deserialize)]
pub struct PtyWriteRequest {
    /// PTY ID
    pub id: String,
    /// Base64-encoded bytes to write to the PTY.
    pub data: String,
}

/// Request to resize a web PTY.
#[derive(Deserialize)]
pub struct PtyResizeRequest {
    /// PTY ID
    pub id: String,
    pub rows: u16,
    pub cols: u16,
}

/// Request to kill a web PTY.
#[derive(Deserialize)]
pub struct PtyKillRequest {
    /// PTY ID
    pub id: String,
}

#[derive(Deserialize)]
pub struct SseTokenQuery {
    pub token: Option<String>,
    /// PTY ID for terminal stream
    pub id: Option<String>,
}

// ── Proxy request types (opencode API proxy) ────────────────────────

/// Model reference for overriding the default model on a per-message basis.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ModelRef {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

/// Request to send a message to a session.
#[derive(Deserialize, Serialize)]
pub struct SendMessageRequest {
    pub parts: Vec<serde_json::Value>,
    /// Optional model override — sent through to the upstream opencode API.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
}

/// Request to execute a slash command on a session.
#[derive(Deserialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    #[serde(default)]
    pub arguments: String,
    pub model: Option<String>,
}

/// Request to reply to a permission request.
#[derive(Deserialize)]
pub struct PermissionReplyRequest {
    /// "once", "always", or "reject"
    pub reply: String,
}

/// Request to reply to a question.
#[derive(Deserialize)]
pub struct QuestionReplyRequest {
    pub answers: Vec<Vec<String>>,
}

/// Request to rename a session.
#[derive(Deserialize)]
pub struct RenameSessionRequest {
    pub title: String,
}

/// Theme preview: name + resolved colors for all 15 fields.
#[derive(Serialize, Clone)]
pub struct ThemePreview {
    pub name: String,
    pub colors: WebThemeColors,
}

/// Request to switch the active theme.
#[derive(Deserialize)]
pub struct SwitchThemeRequest {
    pub name: String,
}

/// SSE query params for session event stream (proxied from opencode).
#[derive(Deserialize)]
pub struct SessionSseQuery {
    pub token: Option<String>,
    pub project_dir: Option<String>,
}

// ── Git API types ───────────────────────────────────────────────────

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

// ── Multi-session dashboard types ───────────────────────────────────

/// A single session entry in the sessions overview, enriched with stats and status.
#[derive(Serialize, Clone)]
pub struct SessionOverviewEntry {
    pub id: String,
    pub title: String,
    #[serde(rename = "parentID")]
    pub parent_id: String,
    pub project_name: String,
    pub project_index: usize,
    pub directory: String,
    pub is_busy: bool,
    pub time: WebSessionTime,
    /// Cost and token usage (None if no stats recorded yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<WebSessionStats>,
}

/// Response for `GET /api/sessions/overview`.
#[derive(Serialize)]
pub struct SessionsOverviewResponse {
    pub sessions: Vec<SessionOverviewEntry>,
    /// Total number of sessions across all projects.
    pub total: usize,
    /// Number of currently busy sessions.
    pub busy_count: usize,
}

/// A node in the session tree (parent/child relationships).
#[derive(Serialize, Clone)]
pub struct SessionTreeNode {
    pub id: String,
    pub title: String,
    pub project_name: String,
    pub project_index: usize,
    pub is_busy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<WebSessionStats>,
    pub children: Vec<SessionTreeNode>,
}

/// Response for `GET /api/sessions/tree`.
#[derive(Serialize)]
pub struct SessionsTreeResponse {
    /// Root-level sessions (sessions without a parent, or whose parent is not known).
    pub roots: Vec<SessionTreeNode>,
    /// Total session count.
    pub total: usize,
}

// ── Agent types ─────────────────────────────────────────────────────

/// An agent entry returned by `GET /api/agents`.
#[derive(Serialize, Clone)]
pub struct AgentEntry {
    pub id: String,
    pub label: String,
    pub description: String,
}

// ── File browsing types ─────────────────────────────────────────────

/// Query params for `GET /api/files?path=...`.
#[derive(Deserialize)]
pub struct FileBrowseQuery {
    /// Path relative to project root (default: ".")
    #[serde(default)]
    pub path: String,
}

/// A single directory entry.
#[derive(Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    /// File size in bytes (0 for directories).
    pub size: u64,
}

/// Response for `GET /api/files`.
#[derive(Serialize)]
pub struct FileBrowseResponse {
    pub path: String,
    pub entries: Vec<FileEntry>,
}

/// Query params for `GET /api/file/read?path=...`.
#[derive(Deserialize)]
pub struct FileReadQuery {
    /// Path relative to project root.
    pub path: String,
}

/// Response for `GET /api/file/read`.
#[derive(Serialize)]
pub struct FileReadResponse {
    pub path: String,
    pub content: String,
    /// Detected language hint (e.g. "rust", "javascript", "python").
    pub language: String,
}

/// Request body for `POST /api/file/write`.
#[derive(Deserialize)]
pub struct FileWriteRequest {
    /// Path relative to project root.
    pub path: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct EditorLspQuery {
    pub path: String,
    pub session_id: String,
    pub line: Option<i64>,
    pub col: Option<i64>,
}

#[derive(Deserialize)]
pub struct EditorFormatRequest {
    pub path: String,
    pub session_id: String,
}

// ── Watcher types ───────────────────────────────────────────────────

/// Request to create or update a session watcher.
#[derive(Deserialize, Clone)]
pub struct WatcherConfigRequest {
    pub session_id: String,
    pub project_idx: usize,
    pub idle_timeout_secs: u64,
    pub continuation_message: String,
    #[serde(default)]
    pub include_original: bool,
    pub original_message: Option<String>,
    #[serde(default = "default_hang_message")]
    pub hang_message: String,
    #[serde(default = "default_hang_timeout")]
    pub hang_timeout_secs: u64,
}

fn default_hang_message() -> String {
    "The previous attempt appears to have stalled. Please retry the task.".to_string()
}

fn default_hang_timeout() -> u64 {
    180
}

/// Response for a single watcher entry.
#[derive(Serialize, Clone, Debug)]
pub struct WatcherConfigResponse {
    pub session_id: String,
    pub project_idx: usize,
    pub idle_timeout_secs: u64,
    pub continuation_message: String,
    pub include_original: bool,
    pub original_message: Option<String>,
    pub hang_message: String,
    pub hang_timeout_secs: u64,
    /// Current watcher status: "idle_countdown", "running", "waiting", "inactive"
    pub status: String,
    /// Seconds since session went idle (if in countdown).
    pub idle_since_secs: Option<u64>,
}

/// A list entry for GET /api/watchers.
#[derive(Serialize, Clone)]
pub struct WatcherListEntry {
    pub session_id: String,
    pub session_title: String,
    pub project_name: String,
    pub idle_timeout_secs: u64,
    pub status: String,
    pub idle_since_secs: Option<u64>,
}

/// Session entry for the watcher modal session picker.
#[derive(Serialize, Clone)]
pub struct WatcherSessionEntry {
    pub session_id: String,
    pub title: String,
    pub project_name: String,
    pub project_idx: usize,
    pub is_current: bool,
    pub is_active: bool,
    pub has_watcher: bool,
}

/// SSE event payload for watcher status changes.
#[derive(Clone, Debug, Serialize)]
pub struct WatcherStatusEvent {
    pub session_id: String,
    /// "created", "deleted", "triggered", "countdown", "cancelled"
    pub action: String,
    pub idle_since_secs: Option<u64>,
}

/// A user message from a session for the original-message picker.
#[derive(Serialize, Clone)]
pub struct WatcherMessageEntry {
    pub role: String,
    pub text: String,
}

// ── Shared Axum state ───────────────────────────────────────────────

// ── File Edit / Diff Review types ───────────────────────────────────

/// A single file edit event tracked during a session.
#[derive(Serialize, Clone, Debug)]
pub struct FileEditEntry {
    /// File path (relative to project root).
    pub path: String,
    /// Content before the edit (snapshot taken on first edit).
    pub original_content: String,
    /// Content after the edit (current file content at time of event).
    pub new_content: String,
    /// ISO 8601 timestamp of the edit event.
    pub timestamp: String,
    /// Sequential edit index (for ordering).
    pub index: usize,
}

/// Response for `GET /api/session/{id}/file-edits`.
#[derive(Serialize)]
pub struct FileEditsResponse {
    pub session_id: String,
    /// All file edits tracked for this session, ordered by time.
    pub edits: Vec<FileEditEntry>,
    /// Total number of files edited.
    pub file_count: usize,
}

// ── Cross-Session Search ────────────────────────────────────────────

/// A single search result — a matching message snippet from a session.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResultEntry {
    pub session_id: String,
    pub session_title: String,
    pub project_name: String,
    pub message_id: String,
    pub role: String,
    /// Text snippet containing the match (truncated).
    pub snippet: String,
    /// Unix timestamp (seconds) of the message.
    pub timestamp: u64,
}

/// Response for GET /api/project/{idx}/search.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResultEntry>,
    pub total: usize,
}

// ── Session Continuity / Activity Feed / Presence ───────────────────

/// A single activity event in a session (fine-grained, real-time).
#[derive(Debug, Clone, Serialize)]
pub struct ActivityEventPayload {
    /// Session this activity belongs to.
    pub session_id: String,
    /// Event category: "file_edit", "tool_call", "terminal", "permission", "question", "status".
    pub kind: String,
    /// Human-readable summary of what happened.
    pub summary: String,
    /// Optional detail (file path, tool name, command, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// ISO 8601 timestamp.
    pub timestamp: String,
}

/// Represents a single connected client's presence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPresence {
    /// Unique client identifier (random per tab/connection).
    pub client_id: String,
    /// "web" or "tui".
    pub interface_type: String,
    /// Which session this client is currently focused on (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_session: Option<String>,
    /// ISO 8601 timestamp of last heartbeat.
    pub last_seen: String,
}

/// Snapshot of all connected clients — broadcast on presence changes.
#[derive(Debug, Clone, Serialize)]
pub struct PresenceSnapshot {
    pub clients: Vec<ClientPresence>,
}

/// Request body for registering/updating presence.
#[derive(Debug, Clone, Deserialize)]
pub struct PresenceRegisterRequest {
    pub client_id: String,
    pub interface_type: String,
    #[serde(default)]
    pub focused_session: Option<String>,
}

/// Request body for deregistering presence.
#[derive(Debug, Clone, Deserialize)]
pub struct PresenceDeregisterRequest {
    pub client_id: String,
}

/// Response for `GET /api/presence`.
#[derive(Debug, Clone, Serialize)]
pub struct PresenceResponse {
    pub clients: Vec<ClientPresence>,
}

/// Recent activity events for a session.
#[derive(Debug, Clone, Serialize)]
pub struct ActivityFeedResponse {
    pub session_id: String,
    pub events: Vec<ActivityEventPayload>,
}

// ─── Missions ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionStatus {
    Planned,
    Active,
    Blocked,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: String,
    pub title: String,
    pub goal: String,
    #[serde(default)]
    pub next_action: String,
    pub status: MissionStatus,
    pub project_index: usize,
    #[serde(default)]
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateMissionRequest {
    pub title: String,
    pub goal: String,
    #[serde(default)]
    pub next_action: String,
    #[serde(default)]
    pub status: Option<MissionStatus>,
    pub project_index: usize,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMissionRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub next_action: Option<String>,
    #[serde(default)]
    pub status: Option<MissionStatus>,
    #[serde(default)]
    pub project_index: Option<usize>,
    #[serde(default)]
    pub session_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MissionsListResponse {
    pub missions: Vec<Mission>,
}

// ─── Personal Memory ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Global,
    Project,
    Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalMemoryItem {
    pub id: String,
    pub label: String,
    pub content: String,
    pub scope: MemoryScope,
    #[serde(default)]
    pub project_index: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePersonalMemoryRequest {
    pub label: String,
    pub content: String,
    pub scope: MemoryScope,
    #[serde(default)]
    pub project_index: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePersonalMemoryRequest {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub scope: Option<MemoryScope>,
    #[serde(default)]
    pub project_index: Option<Option<usize>>,
    #[serde(default)]
    pub session_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonalMemoryListResponse {
    pub memory: Vec<PersonalMemoryItem>,
}

// ─── Autonomy Controls ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyMode {
    Observe,
    Nudge,
    Continue,
    Autonomous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomySettings {
    pub mode: AutonomyMode,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAutonomySettingsRequest {
    pub mode: AutonomyMode,
}

// ─── Routines ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutineTrigger {
    Manual,
    OnSessionIdle,
    DailySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutineAction {
    ReviewMission,
    OpenInbox,
    OpenActivityFeed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineDefinition {
    pub id: String,
    pub name: String,
    pub trigger: RoutineTrigger,
    pub action: RoutineAction,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineRunRecord {
    pub id: String,
    pub routine_id: String,
    pub status: String,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRoutineRequest {
    pub name: String,
    pub trigger: RoutineTrigger,
    pub action: RoutineAction,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRoutineRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub trigger: Option<RoutineTrigger>,
    #[serde(default)]
    pub action: Option<RoutineAction>,
    #[serde(default)]
    pub mission_id: Option<Option<String>>,
    #[serde(default)]
    pub session_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunRoutineRequest {
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutinesListResponse {
    pub routines: Vec<RoutineDefinition>,
    pub runs: Vec<RoutineRunRecord>,
}

// ─── Delegation Board ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationStatus {
    Planned,
    Running,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedWorkItem {
    pub id: String,
    pub title: String,
    pub assignee: String,
    pub scope: String,
    pub status: DelegationStatus,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub subagent_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDelegatedWorkRequest {
    pub title: String,
    pub assignee: String,
    pub scope: String,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub subagent_session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDelegatedWorkRequest {
    #[serde(default)]
    pub status: Option<DelegationStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DelegatedWorkListResponse {
    pub items: Vec<DelegatedWorkItem>,
}

// ─── Workspace Snapshots ──────────────────────────────────────

/// Saved workspace snapshot — captures the full panel/layout state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    /// User-chosen name for this workspace.
    pub name: String,
    /// ISO-8601 timestamp when the snapshot was created.
    pub created_at: String,
    /// Panel visibility states.
    pub panels: WorkspacePanels,
    /// Panel sizes (percentages or pixel values).
    #[serde(default)]
    pub layout: WorkspaceLayout,
    /// Paths of files that were open in the editor.
    #[serde(default)]
    pub open_files: Vec<String>,
    /// Which file was the active/focused one in the editor.
    #[serde(default)]
    pub active_file: Option<String>,
    /// Terminal tabs that were open.
    #[serde(default)]
    pub terminal_tabs: Vec<WorkspaceTerminalTab>,
    /// Active session ID when snapshot was taken.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Git branch that was checked out.
    #[serde(default)]
    pub git_branch: Option<String>,
    /// Whether this is a built-in task template (not user-deletable).
    #[serde(default)]
    pub is_template: bool,
    /// Intent-oriented recipe metadata.
    #[serde(default)]
    pub recipe_description: Option<String>,
    #[serde(default)]
    pub recipe_next_action: Option<String>,
    #[serde(default)]
    pub is_recipe: bool,
}

/// Panel visibility flags within a workspace snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePanels {
    pub sidebar: bool,
    pub terminal: bool,
    pub editor: bool,
    pub git: bool,
}

/// Panel layout sizes within a workspace snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceLayout {
    /// Sidebar width in pixels (0 = use default).
    #[serde(default)]
    pub sidebar_width: u32,
    /// Terminal height in pixels (0 = use default).
    #[serde(default)]
    pub terminal_height: u32,
    /// Side panel width in pixels (0 = use default).
    #[serde(default)]
    pub side_panel_width: u32,
}

/// Terminal tab descriptor within a workspace snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTerminalTab {
    /// Label shown on the tab.
    pub label: String,
    /// Kind of terminal (e.g. "shell", "command").
    #[serde(default)]
    pub kind: String,
}

/// Request body for saving a workspace snapshot.
#[derive(Debug, Clone, Deserialize)]
pub struct SaveWorkspaceRequest {
    pub snapshot: WorkspaceSnapshot,
}

/// Response listing all saved workspaces.
#[derive(Debug, Clone, Serialize)]
pub struct WorkspacesListResponse {
    pub workspaces: Vec<WorkspaceSnapshot>,
}

/// Shared state available to all Axum handlers via `State<ServerState>`.
#[derive(Clone)]
pub struct ServerState {
    /// Independent web state manager (talks directly to opencode API).
    pub web_state: WebStateHandle,
    /// JWT signing secret (random per run).
    pub jwt_secret: Vec<u8>,
    /// Expected username (empty = no auth required).
    pub username: String,
    /// Expected password.
    pub password: String,
    /// Broadcast channel for app events (state changes, busy/idle, etc.).
    pub event_tx: broadcast::Sender<WebEvent>,
    /// Broadcast channel for raw upstream opencode SSE events.
    /// Each value is the raw JSON string from the upstream `/event` stream
    /// (already extracted from the `data:` SSE field).  The web
    /// `session_events_stream` subscribes here instead of opening a separate
    /// upstream connection (the opencode server may limit concurrent SSE
    /// consumers per project).
    pub raw_sse_tx: broadcast::Sender<String>,
    /// Handle to the web PTY manager (independent from TUI PTYs).
    pub pty_mgr: WebPtyHandle,
    /// Shared HTTP client for proxying requests to the opencode server.
    /// Reuses TCP connections across requests (connection pooling).
    pub http_client: reqwest::Client,
    /// Shared neovim socket registry for LSP-backed editor features.
    pub nvim_registry: NvimSocketRegistry,
}
