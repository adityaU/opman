//! API response types — matches TypeScript API module types.

use serde::{Deserialize, Serialize};

// ── State types (api/state.ts) ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    #[serde(rename = "parentID")]
    pub parent_id: String,
    pub directory: String,
    pub time: SessionTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionTime {
    pub created: f64,
    pub updated: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub index: usize,
    pub active_session: Option<String>,
    pub sessions: Vec<SessionInfo>,
    pub git_branch: String,
    pub busy_sessions: Vec<String>,
    /// Sessions that have encountered an error.
    #[serde(default)]
    pub error_sessions: Vec<String>,
    /// Sessions that need user input (pending permission or question).
    #[serde(default)]
    pub input_sessions: Vec<String>,
    /// Sessions with unseen activity (completed/errored while not being viewed).
    #[serde(default)]
    pub unseen_sessions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppState {
    pub projects: Vec<ProjectInfo>,
    pub active_project: usize,
    pub panels: PanelVisibility,
    pub focused: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelVisibility {
    pub sidebar: bool,
    pub terminal_pane: bool,
    pub neovim_pane: bool,
    pub integrated_terminal: bool,
    pub git_panel: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionStats {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(default)]
    pub cache_read: u64,
    #[serde(default)]
    pub cache_write: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeColors {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemePreview {
    pub name: String,
    pub dark: ThemeColors,
    pub light: ThemeColors,
}

/// Both dark and light variants of the active theme (from bootstrap, SSE, switch).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemePair {
    pub dark: ThemeColors,
    pub light: ThemeColors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapData {
    pub theme: Option<ThemePair>,
    pub instance_name: Option<String>,
}

// ── Session & Message API types ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePageResponse {
    pub messages: Vec<super::core::Message>,
    pub has_more: bool,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAttachment {
    pub base64: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingResponse {
    pub permissions: Vec<serde_json::Value>,
    pub questions: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

// ── Project types ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_project: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowseDirsResponse {
    pub path: String,
    pub parent: String,
    pub entries: Vec<DirEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSessionResponse {
    pub session_id: String,
}

// ── PTY types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnPtyResponse {
    pub id: String,
    pub ok: bool,
}

// ── Git types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFileEntry {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusResponse {
    pub branch: String,
    pub staged: Vec<GitFileEntry>,
    pub unstaged: Vec<GitFileEntry>,
    pub untracked: Vec<GitFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogResponse {
    pub commits: Vec<GitLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommitResponse {
    pub hash: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitShowResponse {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
    pub diff: String,
    pub files: Vec<GitShowFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitShowFile {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBranchesResponse {
    pub current: String,
    pub local: Vec<String>,
    pub remote: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffResponse {
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCheckoutResponse {
    pub branch: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRangeDiffResponse {
    pub branch: String,
    pub base: String,
    pub commits: Vec<GitLogEntry>,
    pub diff: String,
    pub files_changed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitContextSummaryResponse {
    pub branch: String,
    pub recent_commits: Vec<GitLogEntry>,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepoEntry {
    pub path: String,
    pub name: String,
    pub branch: String,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitReposResponse {
    pub repos: Vec<GitRepoEntry>,
}

// ── Git pull / stash / gitignore types ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPullResponse {
    pub success: bool,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStashEntry {
    pub index: usize,
    pub reference: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStashResponse {
    pub success: bool,
    pub output: String,
    #[serde(default)]
    pub entries: Vec<GitStashEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitIgnoreResponse {
    pub success: bool,
    pub content: String,
}

// ── File types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBrowseResponse {
    pub path: String,
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadResponse {
    pub path: String,
    pub content: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadResponse {
    pub files: Vec<String>,
}

// ── Editor / LSP types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorLspDiagnostic {
    pub file: String,
    pub lnum: u32,
    pub col: u32,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorLspDiagnosticsResponse {
    pub diagnostics: Vec<EditorLspDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorDefinitionLocation {
    pub file: String,
    pub lnum: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorDefinitionResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<EditorDefinitionLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorHoverResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorFormatResponse {
    pub formatted: String,
}

// ── PTY list types ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyListResponse {
    pub ptys: Vec<PtyEntry>,
}

// ── Session views types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOverviewEntry {
    pub id: String,
    pub title: String,
    #[serde(rename = "parentID")]
    pub parent_id: String,
    pub project_name: String,
    pub project_index: usize,
    pub directory: String,
    pub is_busy: bool,
    pub time: SessionTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<SessionStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsOverviewResponse {
    pub sessions: Vec<SessionOverviewEntry>,
    pub total: usize,
    pub busy_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTreeNode {
    pub id: String,
    pub title: String,
    pub project_name: String,
    pub project_index: usize,
    pub is_busy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<SessionStats>,
    pub children: Vec<SessionTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsTreeResponse {
    pub roots: Vec<SessionTreeNode>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    pub label: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCategory {
    pub name: String,
    pub label: String,
    pub tokens: u64,
    pub pct: f64,
    pub color: String,
    pub items: Vec<ContextItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextWindowResponse {
    pub context_limit: u64,
    pub total_used: u64,
    pub usage_pct: f64,
    pub categories: Vec<ContextCategory>,
    pub estimated_messages_remaining: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditEntry {
    pub path: String,
    pub original_content: String,
    pub new_content: String,
    pub timestamp: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditsResponse {
    pub session_id: String,
    pub edits: Vec<FileEditEntry>,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultEntry {
    pub session_id: String,
    pub session_title: String,
    pub project_name: String,
    pub message_id: String,
    pub role: String,
    pub snippet: String,
    pub timestamp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResultEntry>,
    pub total: usize,
}

// ── Watcher types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherListEntry {
    pub session_id: String,
    pub session_title: String,
    pub project_name: String,
    pub idle_timeout_secs: u64,
    pub status: String,
    pub idle_since_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfigResponse {
    pub session_id: String,
    pub project_idx: usize,
    pub idle_timeout_secs: u64,
    pub continuation_message: String,
    pub include_original: bool,
    pub original_message: Option<String>,
    pub hang_message: String,
    pub hang_timeout_secs: u64,
    pub status: String,
    pub idle_since_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherSessionEntry {
    pub session_id: String,
    pub title: String,
    pub project_name: String,
    pub project_idx: usize,
    pub is_current: bool,
    pub is_active: bool,
    pub has_watcher: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub session_id: String,
    pub kind: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientPresence {
    pub client_id: String,
    pub interface_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_session: Option<String>,
    pub last_seen: String,
}

// ── Mission types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: String,
    pub goal: String,
    pub session_id: String,
    pub project_index: usize,
    pub state: String,
    pub iteration: u32,
    pub max_iterations: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_verdict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_eval_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_history: Option<Vec<EvalRecord>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRecord {
    pub iteration: u32,
    pub verdict: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionsListResponse {
    pub missions: Vec<Mission>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalMemoryItem {
    pub id: String,
    pub label: String,
    pub content: String,
    pub scope: String,
    pub project_index: Option<usize>,
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalMemoryListResponse {
    pub memory: Vec<PersonalMemoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomySettings {
    pub mode: String,
    pub updated_at: String,
}

// ── Workflow types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineDefinition {
    pub id: String,
    pub name: String,
    pub trigger: String,
    pub action: String,
    pub enabled: bool,
    pub cron_expr: Option<String>,
    pub timezone: Option<String>,
    pub target_mode: Option<String>,
    pub session_id: Option<String>,
    pub project_index: Option<usize>,
    pub prompt: Option<String>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub mission_id: Option<String>,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineRunRecord {
    pub id: String,
    pub routine_id: String,
    pub status: String,
    pub summary: String,
    pub target_session_id: Option<String>,
    pub duration_ms: Option<u64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutinesListResponse {
    pub routines: Vec<RoutineDefinition>,
    pub runs: Vec<RoutineRunRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedWorkItem {
    pub id: String,
    pub title: String,
    pub assignee: String,
    pub scope: String,
    pub status: String,
    pub mission_id: Option<String>,
    pub session_id: Option<String>,
    pub subagent_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedWorkListResponse {
    pub items: Vec<DelegatedWorkItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub name: String,
    pub created_at: String,
    pub panels: WorkspacePanels,
    pub layout: WorkspaceLayout,
    pub open_files: Vec<String>,
    pub active_file: Option<String>,
    pub terminal_tabs: Vec<WorkspaceTerminalTab>,
    pub session_id: Option<String>,
    pub git_branch: Option<String>,
    pub is_template: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe_next_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_recipe: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePanels {
    pub sidebar: bool,
    pub terminal: bool,
    pub editor: bool,
    pub git: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    pub sidebar_width: f64,
    pub terminal_height: f64,
    pub side_panel_width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTerminalTab {
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacesListResponse {
    pub workspaces: Vec<WorkspaceSnapshot>,
}

// ── Intelligence types ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: String,
    pub source: String,
    pub title: String,
    pub description: String,
    pub priority: String,
    pub state: String,
    pub created_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mission_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxResponse {
    pub items: Vec<InboxItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantRecommendation {
    pub id: String,
    pub title: String,
    pub rationale: String,
    pub action: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationsResponse {
    pub recommendations: Vec<AssistantRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantCenterStats {
    pub active_missions: usize,
    pub paused_missions: usize,
    pub total_missions: usize,
    pub pending_permissions: usize,
    pub pending_questions: usize,
    pub memory_items: usize,
    pub active_routines: usize,
    pub active_delegations: usize,
    pub workspace_count: usize,
    pub autonomy_mode: String,
}

// ── System types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f64,
    pub mem: u64,
    pub status: String,
    pub disk_read: u64,
    pub disk_write: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub used: u64,
    pub fs_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub mem_total: u64,
    pub mem_used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub cpu_usage: Vec<f64>,
    pub cpu_avg: f64,
    pub uptime_secs: u64,
    pub hostname: String,
    pub load_avg: (f64, f64, f64),
    pub processes: Vec<ProcessInfo>,
    pub process_count: usize,
    pub disks: Vec<DiskInfo>,
    pub networks: Vec<NetworkInfo>,
}
