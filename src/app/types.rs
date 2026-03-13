use crate::pty::PtyInstance;
use crate::ui::layout_manager::PanelId;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum SidebarItem {
    Project(usize),
    NewSession(usize),
    Session(usize, String),
    /// A subagent session shown under an expanded parent session.
    SubAgentSession(usize, String),
    MoreSessions(usize),
    AddProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    AddProject,
    FuzzyPicker,
}

/// The base URL for the managed OpenCode server (set at startup after spawning).
pub static BASE_URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn init_base_url(url: String) {
    BASE_URL.set(url).expect("BASE_URL already initialized");
}

pub fn base_url() -> &'static str {
    BASE_URL
        .get()
        .expect("BASE_URL not initialized — opencode server not started")
}

/// Represents a single managed project with its server and optional PTY.
#[derive(Debug)]
pub struct Project {
    /// Human-readable name (derived from directory basename or config).
    pub name: String,
    /// Absolute path to the project directory.
    pub path: PathBuf,
    /// PTYs keyed by session ID. Switching sessions reuses cached PTYs.
    pub ptys: HashMap<String, PtyInstance>,
    /// Which session's PTY is currently active (key into `ptys`).
    pub active_session: Option<String>,
    /// Per-session resources (shell PTYs, neovim, file snapshots).
    /// Keyed by session ID. Lazily initialized when a session becomes active.
    pub session_resources: HashMap<String, SessionResources>,
    /// PTY running gitui, per-project.
    pub gitui_pty: Option<PtyInstance>,
    /// Cached list of session IDs fetched from the server's REST API.
    pub sessions: Vec<SessionInfo>,
    /// Git branch name (best-effort, may be empty).
    pub git_branch: String,
}

impl Project {
    pub fn active_pty(&self) -> Option<&PtyInstance> {
        self.active_session
            .as_ref()
            .and_then(|sid| self.ptys.get(sid))
    }

    pub fn active_pty_mut(&mut self) -> Option<&mut PtyInstance> {
        let sid = self.active_session.clone();
        sid.and_then(move |s| self.ptys.get_mut(&s))
    }

    /// Get the active session's resources (immutable).
    pub fn active_resources(&self) -> Option<&SessionResources> {
        self.active_session
            .as_ref()
            .and_then(|sid| self.session_resources.get(sid))
    }

    /// Get the active session's resources (mutable).
    pub fn active_resources_mut(&mut self) -> Option<&mut SessionResources> {
        let sid = self.active_session.clone();
        sid.and_then(move |s| self.session_resources.get_mut(&s))
    }

    pub fn active_shell_pty(&self) -> Option<&PtyInstance> {
        self.active_resources().and_then(|r| r.active_shell_pty())
    }

    pub fn active_shell_pty_mut(&mut self) -> Option<&mut PtyInstance> {
        self.active_resources_mut()
            .and_then(|r| r.active_shell_pty_mut())
    }
}
/// Minimal session metadata fetched from the opencode server (or directly from the DB).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionInfo {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, rename = "parentID")]
    pub parent_id: String,
    #[serde(default)]
    pub directory: String,
    #[serde(default)]
    pub time: SessionTime,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionTime {
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub updated: u64,
}

/// Per-session resources: terminal tabs, neovim, and file snapshots.
/// These are lazily initialized when a session becomes active.
#[derive(Debug)]
pub struct SessionResources {
    /// PTYs running the user's shell (integrated terminal tabs).
    pub shell_ptys: Vec<PtyInstance>,
    /// Index of the active shell tab.
    pub active_shell_tab: usize,
    /// PTY running neovim for this session.
    pub neovim_pty: Option<PtyInstance>,
    /// Snapshot of file contents *before* the latest edit.
    /// Keyed by absolute file path. Used by follow-edits to compute
    /// per-edit diffs instead of cumulative diffs from HEAD.
    pub file_snapshots: HashMap<String, String>,
}

impl SessionResources {
    pub fn new() -> Self {
        Self {
            shell_ptys: Vec::new(),
            active_shell_tab: 0,
            neovim_pty: None,
            file_snapshots: HashMap::new(),
        }
    }

    pub fn active_shell_pty(&self) -> Option<&PtyInstance> {
        self.shell_ptys.get(self.active_shell_tab)
    }

    pub fn active_shell_pty_mut(&mut self) -> Option<&mut PtyInstance> {
        self.shell_ptys.get_mut(self.active_shell_tab)
    }
}
/// Per-session token/cost statistics, updated via SSE `message.updated` events.
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    /// Accumulated cost in USD across all assistant messages in this session.
    pub cost: f64,
    /// Total input tokens from the latest assistant message.
    pub input_tokens: u64,
    /// Total output tokens from the latest assistant message.
    pub output_tokens: u64,
    /// Total reasoning tokens from the latest assistant message.
    pub reasoning_tokens: u64,
    /// Cache read tokens from the latest assistant message.
    pub cache_read: u64,
    /// Cache write tokens from the latest assistant message.
    pub cache_write: u64,
    /// Previous total token count (before the latest update).
    /// Used by the status bar to show an up/down arrow indicating direction of change.
    pub prev_total_tokens: u64,
}

impl SessionStats {
    /// Total tokens used (input + output + reasoning + cache read + cache write).
    #[allow(dead_code)]
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens
            + self.output_tokens
            + self.reasoning_tokens
            + self.cache_read
            + self.cache_write
    }
}

/// Context window limit for the active model/provider.
#[derive(Debug, Clone, Default)]
pub struct ModelLimits {
    /// Maximum context window size in tokens.
    pub context_window: u64,
}

/// A single todo item from the opencode session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: String,
    pub priority: String,
}

/// A permission request from the AI agent (e.g. edit, bash, read).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionRequest {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(default)]
    pub permission: String,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// A question asked by the AI agent via the question tool.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionRequest {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(default)]
    pub questions: Vec<QuestionInfo>,
}

/// A single question within a question request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionInfo {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub options: Vec<QuestionOption>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default)]
    pub custom: bool,
}

/// A single option within a question.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionOption {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
}

/// State for the todo panel overlay.
pub struct TodoPanelState {
    pub todos: Vec<TodoItem>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub session_id: String,
    pub editing: Option<EditingState>,
    /// Set to true when the user modifies any todo. Used to decide
    /// whether to send a system message when the panel is closed.
    pub dirty: bool,
}

/// State for inline editing within the todo panel.
pub struct EditingState {
    /// None = adding new todo, Some(i) = editing existing todo at index i.
    pub index: Option<usize>,
    pub buffer: String,
    pub cursor_pos: usize,
    pub priority: String,
}

/// Mouse-based text selection in a terminal panel.
#[derive(Debug, Clone)]
pub struct TerminalSelection {
    pub panel_id: PanelId,
    pub start_row: u16,
    pub start_col: u16,
    pub end_row: u16,
    pub end_col: u16,
}

impl TodoPanelState {
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.todos.is_empty() && self.selected < self.todos.len() - 1 {
            self.selected += 1;
        }
    }
}

/// A routine definition for TUI display purposes.
/// Mirrors the web-state `RoutineDefinition` but uses plain strings for
/// trigger/action so the UI renderer doesn't depend on web types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoutineItem {
    pub id: String,
    pub name: String,
    pub trigger: String,
    pub action: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub cron_expr: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub target_mode: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub last_run_at: Option<String>,
    #[serde(default)]
    pub next_run_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl RoutineItem {
    /// Convert a web-state `RoutineDefinition` into a TUI `RoutineItem`.
    pub fn from_definition(def: &crate::web::types::RoutineDefinition) -> Self {
        Self {
            id: def.id.clone(),
            name: def.name.clone(),
            trigger: serde_json::to_value(&def.trigger)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| format!("{:?}", def.trigger)),
            action: serde_json::to_value(&def.action)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| format!("{:?}", def.action)),
            enabled: def.enabled,
            cron_expr: def.cron_expr.clone(),
            prompt: def.prompt.clone(),
            target_mode: def.target_mode.as_ref().map(|tm| {
                serde_json::to_value(tm)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| format!("{:?}", tm))
            }),
            session_id: def.session_id.clone(),
            provider_id: def.provider_id.clone(),
            model_id: def.model_id.clone(),
            last_run_at: def.last_run_at.clone(),
            next_run_at: def.next_run_at.clone(),
            last_error: def.last_error.clone(),
        }
    }
}

/// State for the inline routine editor in the TUI panel.
pub struct RoutineEditState {
    /// None = creating new, Some(id) = editing existing
    pub routine_id: Option<String>,
    /// Which field is focused (0=name, 1=trigger, 2=prompt, 3=target_mode, 4=cron, 5=enabled)
    pub focused_field: usize,
    /// Total number of fields
    pub field_count: usize,
    /// Current field values
    pub name: String,
    pub trigger: String, // "manual" or "scheduled"
    pub prompt: String,
    pub target_mode: String, // "existing_session" or "new_session"
    pub cron_expr: String,
    pub enabled: bool,
}

impl RoutineEditState {
    pub fn new_create() -> Self {
        Self {
            routine_id: None,
            focused_field: 0,
            field_count: 6,
            name: String::new(),
            trigger: "scheduled".to_string(),
            prompt: String::new(),
            target_mode: "new_session".to_string(),
            cron_expr: "0 0 */6 * * *".to_string(),
            enabled: true,
        }
    }

    pub fn from_routine(routine: &RoutineItem) -> Self {
        Self {
            routine_id: Some(routine.id.clone()),
            focused_field: 0,
            field_count: 6,
            name: routine.name.clone(),
            trigger: routine.trigger.clone(),
            prompt: routine.prompt.clone().unwrap_or_default(),
            target_mode: routine
                .target_mode
                .clone()
                .unwrap_or_else(|| "new_session".to_string()),
            cron_expr: routine
                .cron_expr
                .clone()
                .unwrap_or_else(|| "0 0 */6 * * *".to_string()),
            enabled: routine.enabled,
        }
    }

    pub fn focus_next(&mut self) {
        if self.focused_field < self.field_count - 1 {
            self.focused_field += 1;
        }
    }

    pub fn focus_prev(&mut self) {
        if self.focused_field > 0 {
            self.focused_field -= 1;
        }
    }
}

/// State for the routine panel overlay.
pub struct RoutinePanelState {
    pub routines: Vec<RoutineItem>,
    pub selected: usize,
    pub scroll_offset: usize,
    /// Set to Some(routine_id) while a run is in progress.
    pub running: Option<String>,
    /// True while the initial fetch is in-flight.
    pub loading: bool,
    /// True when the detail pane is shown for the selected routine.
    pub show_detail: bool,
    /// When Some, the panel is in create/edit mode.
    pub editing: Option<RoutineEditState>,
    /// Set to Some(routine_id) when waiting for delete confirmation.
    pub confirm_delete: Option<String>,
}

impl RoutinePanelState {
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.routines.is_empty() && self.selected < self.routines.len() - 1 {
            self.selected += 1;
        }
    }
}

/// State for terminal text search overlay.
#[derive(Debug, Clone)]
pub struct TerminalSearchState {
    /// The search query string.
    pub query: String,
    /// Cursor position within the query string.
    pub cursor: usize,
    /// All match positions as (row, col, length) tuples.
    pub matches: Vec<(usize, usize, usize)>,
    /// Index of the currently highlighted match.
    pub current_match: usize,
}
