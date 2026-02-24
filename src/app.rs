use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::command_palette::CommandPalette;
use crate::config::{Config, ProjectEntry};
use crate::pty::PtyInstance;
use crate::theme::{color_to_hex, ThemeColors};
use crate::theme_gen;
use crate::ui::fuzzy_picker::FuzzyPickerState;
use crate::ui::layout_manager::{LayoutManager, PanelId};
use crate::vim_mode::{EscapeTracker, VimMode};
use crate::which_key::{RuntimeKeyBinding, WhichKeyState};

/// Events sent from background tokio tasks back to the main event loop.
/// The event loop calls `try_recv()` each tick and dispatches to `App::handle_background_event`.
pub enum BackgroundEvent {
    /// A PTY was successfully spawned in a background (spawn_blocking) task.
    PtySpawned {
        project_idx: usize,
        session_id: String,
        pty: PtyInstance,
    },
    /// Sessions were fetched for a project.
    SessionsFetched {
        project_idx: usize,
        sessions: Vec<SessionInfo>,
    },
    /// Session fetch failed (non-fatal, just skip).
    SessionFetchFailed { project_idx: usize },
    /// A session was selected via the API (pending_session_select completed).
    SessionSelected {
        project_idx: usize,
        session_id: String,
    },
    /// A project was fully activated (server healthy + PTY spawned).
    ProjectActivated { project_idx: usize },
    /// SSE: a new session was created on the server.
    SseSessionCreated {
        project_idx: usize,
        session: SessionInfo,
    },
    /// SSE: a session was updated (title changed, etc.).
    SseSessionUpdated {
        project_idx: usize,
        session: SessionInfo,
    },
    /// SSE: a session was deleted on the server.
    SseSessionDeleted {
        project_idx: usize,
        session_id: String,
    },
    /// SSE: a session became idle.
    SseSessionIdle {
        #[allow(dead_code)]
        project_idx: usize,
        session_id: String,
    },
    /// SSE: a session became busy (actively processing).
    SseSessionBusy { session_id: String },
    /// SSE: a file was edited by the AI agent.
    SseFileEdited {
        project_idx: usize,
        file_path: String,
    },
    /// Todos fetched via REST API.
    TodosFetched {
        session_id: String,
        todos: Vec<TodoItem>,
    },
    /// SSE: todo list updated for a session.
    SseTodoUpdated {
        session_id: String,
        todos: Vec<TodoItem>,
    },
    /// SSE: message.updated with cost/token data for a session.
    SseMessageUpdated {
        session_id: String,
        cost: f64,
        input_tokens: u64,
        output_tokens: u64,
        reasoning_tokens: u64,
        cache_read: u64,
        cache_write: u64,
    },
    /// Provider model limits fetched from REST API.
    ModelLimitsFetched {
        project_idx: usize,
        context_window: u64,
    },
    /// MCP socket request from a bridge process (terminal tool invocation).
    McpSocketRequest {
        project_idx: usize,
        session_id: String,
        pending: crate::mcp::PendingSocketRequest,
    },
}

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
    pub panel_id: crate::ui::layout_manager::PanelId,
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

/// A single entry in the cross-project session selector.
#[derive(Debug, Clone)]
pub struct SessionSelectorEntry {
    pub project_name: String,
    pub project_idx: usize,
    pub session: SessionInfo,
}

/// State for the cross-project session selector overlay.
pub struct SessionSelectorState {
    pub entries: Vec<SessionSelectorEntry>,
    pub query: String,
    pub cursor_pos: usize,
    pub selected: usize,
    pub scroll_offset: usize,
    pub filtered: Vec<usize>,
}

impl SessionSelectorState {
    /// Recompute filtered indices based on current query.
    pub fn update_filter(&mut self) {
        let query = self.query.to_lowercase();
        if query.is_empty() {
            self.filtered = (0..self.entries.len()).collect();
        } else {
            self.filtered = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    let haystack = format!("{} {}", e.project_name, e.session.title).to_lowercase();
                    haystack.contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
        self.scroll_offset = 0;
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else if !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() {
            if self.selected < self.filtered.len() - 1 {
                self.selected += 1;
            } else {
                self.selected = 0;
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.query.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.update_filter();
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.query[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.replace_range(prev..self.cursor_pos, "");
            self.cursor_pos = prev;
            self.update_filter();
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.query[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.query.len() {
            self.cursor_pos = self.query[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.query.len());
        }
    }
}

/// Connection status of a project's server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Error,
}

pub struct App {
    pub projects: Vec<Project>,
    pub active_project: usize,
    pub layout: LayoutManager,
    pub should_quit: bool,
    pub sidebar_selection: usize,
    pub sidebar_cursor: usize,
    pub sidebar_pending_g: bool,
    pub config: Config,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub input_cursor: usize,
    pub pending_remove: Option<usize>,
    pub confirm_delete: Option<usize>,
    pub completions: Vec<String>,
    pub completion_selected: usize,
    pub completions_visible: bool,
    pub show_cheatsheet: bool,
    pub theme: ThemeColors,

    pub session_search_mode: bool,
    pub session_search_buffer: String,
    pub session_search_cursor: usize,
    pub session_search_all: Vec<SessionInfo>,
    pub session_search_results: Vec<SessionInfo>,
    pub session_search_selected: usize,
    pub pinned_sessions: HashMap<usize, Vec<String>>,
    pub pending_session_select: Option<(usize, String)>,

    pub fuzzy_picker: Option<FuzzyPickerState>,

    /// Which project index currently has its sessions expanded in the sidebar.
    /// Only one project can show sessions at a time.
    pub sessions_expanded_for: Option<usize>,

    /// Set by input handler when user triggers "New Session".
    /// Consumed by main event loop to spawn PTY once.
    pub pending_new_session: Option<usize>,
    /// Persists until SSE session.created arrives for this project.
    /// SSE handler auto-selects the new session, then clears this.
    pub awaiting_new_session: Option<usize>,

    /// Session IDs that are currently active/running (not idle).
    /// Tracked via SSE session.created/updated (active) and session.idle (inactive).
    pub active_sessions: HashSet<String>,
    /// Sine-wave phase (0.0–2π) for pulsating active session dot. Updated every frame.
    pub pulse_phase: f64,

    /// Which parent session ID currently has its subagent sessions expanded in the sidebar.
    /// Press `o` on a session to expand/collapse its subagent children.
    pub subagents_expanded_for: Option<String>,

    pub vim_mode: VimMode,
    pub escape_tracker: EscapeTracker,
    pub command_palette: CommandPalette,
    pub which_key: WhichKeyState,
    /// Full runtime keymap tree built from config at startup.
    pub runtime_keymap: Vec<RuntimeKeyBinding>,

    /// When true, only the focused panel is rendered (fullscreen).
    pub zen_mode: bool,
    /// Snapshot of panel visibility and focus before entering zen mode, so we can restore on exit.
    pub pre_zen_state: Option<([bool; 5], PanelId)>,

    /// When true, panels have been popped out into external OS terminal windows.
    pub popout_mode: bool,
    /// Snapshot of panel visibility and focus before entering pop-out mode.
    pub pre_popout_state: Option<([bool; 5], PanelId)>,
    /// Handles to spawned external terminal window processes (killed on toggle-back or exit).
    pub popout_windows: Vec<std::process::Child>,

    /// When true, the config panel overlay is shown.
    pub show_config_panel: bool,
    /// Currently selected row in the config panel.
    pub config_panel_selected: usize,

    /// Cross-project session selector overlay state.
    pub session_selector: Option<SessionSelectorState>,

    /// Todo panel overlay state (None = closed).
    pub todo_panel: Option<TodoPanelState>,

    /// Per-session token/cost statistics keyed by session ID.
    pub session_stats: HashMap<String, SessionStats>,

    /// Context window limits keyed by project index.
    pub model_limits: HashMap<usize, ModelLimits>,

    /// When true, the Neovim MCP server is enabled. This disables follow-edits
    /// (since the AI edits through Neovim directly) and OpenCode's native
    /// edit/write tools are denied via opencode.json permissions.
    pub neovim_mcp_enabled: bool,

    /// Sender half of the background event channel.
    /// Background tasks clone this to send events back to the main loop.
    pub bg_tx: mpsc::UnboundedSender<BackgroundEvent>,

    /// Toast notification message and display timestamp.
    pub toast_message: Option<(String, std::time::Instant)>,
    /// Active terminal text selection.
    pub terminal_selection: Option<TerminalSelection>,
    /// Terminal search state (Ctrl+F search overlay).
    pub terminal_search: Option<TerminalSearchState>,
    /// Context input overlay state (multi-line text entry for OpenCode sessions).
    pub context_input: Option<ContextInputState>,
    /// Maps session IDs to the project index that owns them, preventing cross-project duplication.
    pub session_ownership: std::collections::HashMap<String, usize>,
}

/// State for context input overlay (multi-line text entry for OpenCode sessions).
#[derive(Debug, Clone)]
pub struct ContextInputState {
    /// Lines of text in the input buffer.
    pub lines: Vec<String>,
    /// Current cursor row (line index).
    pub cursor_row: usize,
    /// Current cursor column (byte offset within current line).
    pub cursor_col: usize,
}

impl ContextInputState {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    pub fn insert_char(&mut self, c: char) {
        self.lines[self.cursor_row].insert(self.cursor_col, c);
        self.cursor_col += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        let rest = self.lines[self.cursor_row][self.cursor_col..].to_string();
        self.lines[self.cursor_row].truncate(self.cursor_col);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, rest);
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let prev = self.lines[self.cursor_row][..self.cursor_col]
                .chars()
                .last()
                .unwrap_or(' ');
            self.cursor_col -= prev.len_utf8();
            self.lines[self.cursor_row].remove(self.cursor_col);
        } else if self.cursor_row > 0 {
            let line = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&line);
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_col > 0 {
            let prev = self.lines[self.cursor_row][..self.cursor_col]
                .chars()
                .last()
                .unwrap_or(' ');
            self.cursor_col -= prev.len_utf8();
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
        }
    }

    pub fn cursor_right(&mut self) {
        let line_len = self.lines[self.cursor_row].len();
        if self.cursor_col < line_len {
            let next = self.lines[self.cursor_row][self.cursor_col..]
                .chars()
                .next()
                .unwrap_or(' ');
            self.cursor_col += next.len_utf8();
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn cursor_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
        }
    }

    pub fn cursor_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
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

impl App {
    /// Create a new `App` from a loaded configuration.
    pub fn new(config: Config, bg_tx: mpsc::UnboundedSender<BackgroundEvent>) -> Self {
        let projects: Vec<Project> = config
            .projects
            .iter()
            .map(|entry| Project {
                name: entry.name.clone(),
                path: std::fs::canonicalize(&entry.path)
                    .unwrap_or_else(|_| PathBuf::from(&entry.path)),
                ptys: HashMap::new(),
                active_session: None,
                session_resources: HashMap::new(),
                gitui_pty: None,
                sessions: Vec::new(),
                git_branch: String::new(),
            })
            .collect();
        let theme = crate::theme::load_theme();

        let runtime_keymap = crate::which_key::build_keymap(&config.keybindings);
        let space_children = crate::which_key::build_space_children(&config.keybindings);
        let command_palette = CommandPalette::new(&config.keybindings);

        Self {
            active_project: 0,
            layout: LayoutManager::new(),
            should_quit: false,
            sidebar_selection: 0,
            sidebar_cursor: 0,
            sidebar_pending_g: false,
            projects,
            config,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            input_cursor: 0,
            pending_remove: None,
            confirm_delete: None,
            completions: Vec::new(),
            completion_selected: 0,
            completions_visible: false,
            show_cheatsheet: false,
            theme,
            session_search_mode: false,
            session_search_buffer: String::new(),
            session_search_cursor: 0,
            session_search_all: Vec::new(),
            session_search_results: Vec::new(),
            session_search_selected: 0,
            pinned_sessions: HashMap::new(),
            pending_session_select: None,
            fuzzy_picker: None,
            sessions_expanded_for: None,
            pending_new_session: None,
            awaiting_new_session: None,
            active_sessions: HashSet::new(),
            pulse_phase: 0.0,
            subagents_expanded_for: None,
            vim_mode: VimMode::Normal,
            escape_tracker: EscapeTracker::new(),
            command_palette,
            which_key: WhichKeyState::new(space_children),
            runtime_keymap,
            zen_mode: false,
            pre_zen_state: None,
            popout_mode: false,
            pre_popout_state: None,
            popout_windows: Vec::new(),
            show_config_panel: false,
            config_panel_selected: 0,
            session_selector: None,
            todo_panel: None,
            session_stats: HashMap::new(),
            model_limits: HashMap::new(),
            neovim_mcp_enabled: false,
            bg_tx,
            toast_message: None,
            terminal_selection: None,
            terminal_search: None,
            context_input: None,
            session_ownership: std::collections::HashMap::new(),
        }
    }

    /// Returns the currently active project, if any.
    pub fn active_project(&self) -> Option<&Project> {
        self.projects.get(self.active_project)
    }

    /// Returns a mutable reference to the currently active project.
    pub fn active_project_mut(&mut self) -> Option<&mut Project> {
        self.projects.get_mut(self.active_project)
    }

    /// Switch the active project by index.
    pub fn switch_project(&mut self, index: usize) {
        if index < self.projects.len() {
            self.active_project = index;
            self.resize_all_ptys();
        }
    }

    pub fn resize_all_ptys(&mut self) {
        let area = self.layout.last_area;
        self.layout.compute_rects(area);

        let term_rect = self.layout.panel_rect(PanelId::TerminalPane);
        let shell_rect = self.layout.panel_rect(PanelId::IntegratedTerminal);
        let nvim_rect = self.layout.panel_rect(PanelId::NeovimPane);
        let git_rect = self.layout.panel_rect(PanelId::GitPanel);

        if let Some(project) = self.projects.get_mut(self.active_project) {
            if let (Some(pty), Some(rect)) = (project.active_pty_mut(), term_rect) {
                if rect.width > 0 && rect.height > 0 {
                    let _ = pty.resize(rect.height, rect.width);
                }
            }
            if let Some(resources) = project.active_resources_mut() {
                if let Some(rect) = shell_rect {
                    if rect.width > 0 && rect.height > 0 {
                        // Reserve 1 line for tab bar
                        let content_height = rect.height.saturating_sub(1).max(1);
                        for shell_pty in &mut resources.shell_ptys {
                            let _ = shell_pty.resize(content_height, rect.width);
                        }
                    }
                }
                if let (Some(ref mut nvim_pty), Some(rect)) = (&mut resources.neovim_pty, nvim_rect)
                {
                    if rect.width > 0 && rect.height > 0 {
                        let _ = nvim_pty.resize(rect.height, rect.width);
                    }
                }
            }
            if let (Some(ref mut gitui_pty), Some(rect)) = (&mut project.gitui_pty, git_rect) {
                if rect.width > 0 && rect.height > 0 {
                    let _ = gitui_pty.resize(rect.height, rect.width);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn toggle_sidebar(&mut self) {
        self.layout.toggle_visible(PanelId::Sidebar);
        self.resize_all_ptys();
    }

    #[allow(dead_code)]
    pub fn toggle_focus(&mut self) {
        let panels = self.layout.visible_panels();
        if panels.len() < 2 {
            return;
        }
        let idx = panels
            .iter()
            .position(|&p| p == self.layout.focused)
            .unwrap_or(0);
        let next = (idx + 1) % panels.len();
        self.layout.focused = panels[next];
    }

    pub fn toggle_cheatsheet(&mut self) {
        self.show_cheatsheet = !self.show_cheatsheet;
    }

    /// Close the todo panel. If the user modified any todos (dirty flag),
    /// send a system message to the AI session so it re-reads the list.
    pub fn close_todo_panel(&mut self) {
        if let Some(panel) = self.todo_panel.take() {
            if panel.dirty {
                let session_id = panel.session_id.clone();
                info!(
                    session_id,
                    "Todo panel closed with changes, sending system message"
                );

                let proj_dir = self
                    .projects
                    .iter()
                    .find(|p| p.active_session.as_deref() == Some(&session_id))
                    .map(|p| p.path.to_string_lossy().to_string());

                info!(
                    ?proj_dir,
                    "Resolved project directory for todo system message"
                );

                if let Some(proj_dir) = proj_dir {
                    let base = base_url().to_string();
                    info!(base, session_id, proj_dir, "Sending todo system message");
                    tokio::spawn(async move {
                        let client = crate::api::ApiClient::new();
                        let msg = "[SYSTEM REMINDER - TODO CONTINUATION] The todo list has been \
                                   updated. Re-read your todos and adjust your work plan accordingly. \
                                   Mark completed items done and continue with the next pending task.";
                        match client
                            .send_system_message_async(&base, &proj_dir, &session_id, msg)
                            .await
                        {
                            Ok(()) => info!("Todo system message sent successfully"),
                            Err(e) => {
                                tracing::error!("Failed to send todo continuation prompt: {e}")
                            }
                        }
                    });
                } else {
                    tracing::warn!(
                        session_id,
                        "Could not find project for session, system message not sent"
                    );
                }
            } else {
                debug!("Todo panel closed without changes");
            }
        }
    }

    #[allow(dead_code)]
    pub fn terminal_inner_size(&self, total_rows: u16, total_cols: u16) -> (u16, u16) {
        if let Some(rect) = self.layout.panel_rect(PanelId::TerminalPane) {
            (rect.height, rect.width)
        } else {
            (total_rows.saturating_sub(1), total_cols)
        }
    }

    #[allow(dead_code)]
    pub fn shell_terminal_inner_size(&self, _total_rows: u16, _total_cols: u16) -> (u16, u16) {
        if let Some(rect) = self.layout.panel_rect(PanelId::IntegratedTerminal) {
            (rect.height, rect.width)
        } else {
            (0, 0)
        }
    }

    #[allow(dead_code)]
    pub fn terminal_pane_offset(&self) -> u16 {
        self.layout
            .panel_rect(PanelId::TerminalPane)
            .map(|r| r.x)
            .unwrap_or(0)
    }

    pub fn add_project(&mut self, entry: ProjectEntry) {
        let project = Project {
            name: entry.name.clone(),
            path: std::fs::canonicalize(&entry.path).unwrap_or_else(|_| PathBuf::from(&entry.path)),
            ptys: HashMap::new(),
            active_session: None,
            session_resources: HashMap::new(),
            gitui_pty: None,
            sessions: Vec::new(),
            git_branch: String::new(),
        };
        self.projects.push(project);
        self.config.projects.push(entry);
    }

    pub fn start_add_project(&mut self) {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let existing: Vec<String> = self
            .projects
            .iter()
            .map(|p| p.path.to_string_lossy().to_string())
            .collect();
        self.fuzzy_picker = Some(FuzzyPickerState::new_with_existing(home, existing));
        self.input_mode = InputMode::FuzzyPicker;
    }

    /// Cancel the fuzzy picker and return to normal mode.
    pub fn cancel_fuzzy_picker(&mut self) {
        self.fuzzy_picker = None;
        self.input_mode = InputMode::Normal;
    }

    /// Confirm the fuzzy picker selection and add the project.
    pub fn confirm_fuzzy_add_project(&mut self) -> Result<()> {
        let selected_path = self
            .fuzzy_picker
            .as_ref()
            .and_then(|fp| fp.selected_path())
            .map(|s| s.to_string());

        self.fuzzy_picker = None;
        self.input_mode = InputMode::Normal;

        let path_str = match selected_path {
            Some(p) => p,
            None => return Ok(()),
        };

        // Expand ~ to home directory
        let expanded = if path_str.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.join(&path_str[1..].trim_start_matches('/'))
                    .to_string_lossy()
                    .to_string()
            } else {
                path_str.clone()
            }
        } else {
            path_str.clone()
        };

        let path = PathBuf::from(&expanded);
        if !path.is_dir() {
            return Ok(());
        }

        // If project already exists, switch to it instead of adding
        for (i, project) in self.projects.iter().enumerate() {
            if project.path == path {
                self.switch_project(i);
                return Ok(());
            }
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());

        let entry = ProjectEntry {
            name,
            path: path_str,
            terminal_command: None,
        };
        self.add_project(entry);
        self.config.save()?;
        Ok(())
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.clear_completions();
    }

    pub fn confirm_add_project(&mut self) -> Result<()> {
        let raw = self.input_buffer.trim().to_string();
        let path_str = self.expand_tilde(&raw);
        let path = PathBuf::from(&path_str);

        if !path.is_dir() {
            self.cancel_input();
            return Ok(());
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());

        let entry = ProjectEntry {
            name,
            path: path_str,
            terminal_command: None,
        };
        self.add_project(entry);
        self.config.save()?;

        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.clear_completions();
        Ok(())
    }

    pub fn remove_project(&mut self, index: usize) -> Result<()> {
        if index >= self.projects.len() {
            return Ok(());
        }

        let mut project = self.projects.remove(index);

        for (_, pty) in project.ptys.iter_mut() {
            let _ = pty.kill();
        }
        project.ptys.clear();
        drop(project);

        self.config.projects.remove(index);
        self.config.save()?;

        if self.projects.is_empty() {
            self.active_project = 0;
            self.sidebar_selection = 0;
            self.sidebar_cursor = 0;
        } else {
            if self.active_project >= self.projects.len() {
                self.active_project = self.projects.len().saturating_sub(1);
            }
            let max = self.projects.len().saturating_sub(1);
            self.sidebar_selection = self.sidebar_selection.min(max);
            self.sidebar_cursor = self.sidebar_cursor.min(max);
        }

        Ok(())
    }

    /// Derive the server status for a given project.
    /// With the shared server architecture, we always report Running.
    pub fn project_server_status(&self, _index: usize) -> ServerStatus {
        ServerStatus::Running
    }

    pub fn ensure_shell_pty(&mut self) {
        let index = self.active_project;
        if index >= self.projects.len() {
            return;
        }
        // Need an active session to attach resources to
        let sid = match self.projects[index].active_session.clone() {
            Some(s) => s,
            None => return,
        };
        // Ensure at least one shell tab exists for the active session
        {
            let resources = self.projects[index]
                .session_resources
                .entry(sid.clone())
                .or_insert_with(SessionResources::new);
            if !resources.shell_ptys.is_empty() {
                return;
            }
        }
        let shell_rows = self
            .layout
            .panel_rect(PanelId::IntegratedTerminal)
            .map(|r| (r.height.saturating_sub(1).max(2), r.width.max(2))) // Reserve 1 line for tab bar
            .unwrap_or((24, 80));
        let theme_envs = self.theme.pty_env_vars();
        let td = theme_gen::theme_dir();

        // Determine command: project-specific > global default > $SHELL > /bin/bash
        let command = self
            .config
            .projects
            .get(index)
            .and_then(|e| e.terminal_command.as_deref())
            .or(self.config.settings.default_terminal_command.as_deref());

        match PtyInstance::spawn_shell(
            shell_rows.0,
            shell_rows.1,
            &self.projects[index].path,
            &theme_envs,
            Some(&td),
            command,
            None,
        ) {
            Ok(shell) => {
                let resources = self.projects[index]
                    .session_resources
                    .entry(sid)
                    .or_insert_with(SessionResources::new);
                resources.shell_ptys.push(shell);
                resources.active_shell_tab = 0;
            }
            Err(e) => tracing::warn!(
                project = %self.projects[index].name,
                "Failed to spawn shell PTY: {}", e
            ),
        }
    }
    pub fn add_shell_tab(&mut self) {
        let index = self.active_project;
        if index >= self.projects.len() {
            return;
        }
        let sid = match self.projects[index].active_session.clone() {
            Some(s) => s,
            None => return,
        };
        let shell_rows = self
            .layout
            .panel_rect(PanelId::IntegratedTerminal)
            .map(|r| (r.height.saturating_sub(1).max(2), r.width.max(2))) // Reserve 1 line for tab bar
            .unwrap_or((24, 80));
        let theme_envs = self.theme.pty_env_vars();
        let td = theme_gen::theme_dir();

        let command = self
            .config
            .projects
            .get(index)
            .and_then(|e| e.terminal_command.as_deref())
            .or(self.config.settings.default_terminal_command.as_deref());

        match PtyInstance::spawn_shell(
            shell_rows.0,
            shell_rows.1,
            &self.projects[index].path,
            &theme_envs,
            Some(&td),
            command,
            None,
        ) {
            Ok(shell) => {
                let resources = self.projects[index]
                    .session_resources
                    .entry(sid)
                    .or_insert_with(SessionResources::new);
                resources.shell_ptys.push(shell);
                resources.active_shell_tab = resources.shell_ptys.len() - 1;
            }
            Err(e) => tracing::warn!(
                project = %self.projects[index].name,
                "Failed to spawn shell tab PTY: {}", e
            ),
        }
    }

    pub fn ensure_neovim_pty(&mut self) {
        let index = self.active_project;
        if index >= self.projects.len() {
            return;
        }
        let sid = match self.projects[index].active_session.clone() {
            Some(s) => s,
            None => return,
        };
        {
            let resources = self.projects[index]
                .session_resources
                .entry(sid.clone())
                .or_insert_with(SessionResources::new);
            if resources.neovim_pty.is_some() {
                return;
            }
        }
        let nvim_size = self
            .layout
            .panel_rect(PanelId::NeovimPane)
            .map(|r| (r.height.max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let theme_envs = self.theme.pty_env_vars();
        let td = theme_gen::theme_dir();
        match PtyInstance::spawn_neovim(
            nvim_size.0,
            nvim_size.1,
            &self.projects[index].path,
            &theme_envs,
            Some(&td),
        ) {
            Ok(nvim) => {
                let resources = self.projects[index]
                    .session_resources
                    .entry(sid)
                    .or_insert_with(SessionResources::new);
                resources.neovim_pty = Some(nvim);
            }
            Err(e) => tracing::warn!(
                project = %self.projects[index].name,
                "Failed to spawn neovim PTY: {}", e
            ),
        }
    }

    pub fn ensure_gitui_pty(&mut self) {
        let index = self.active_project;
        if index >= self.projects.len() {
            return;
        }
        if self.projects[index].gitui_pty.is_some() {
            return;
        }
        let git_size = self
            .layout
            .panel_rect(PanelId::GitPanel)
            .map(|r| (r.height.max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let theme_path = theme_gen::theme_dir().join("gitui/opencode.ron");
        let theme_ref = if theme_path.exists() {
            Some(theme_path.as_path())
        } else {
            None
        };
        match PtyInstance::spawn_gitui(
            git_size.0,
            git_size.1,
            &self.projects[index].path,
            theme_ref,
        ) {
            Ok(pty) => self.projects[index].gitui_pty = Some(pty),
            Err(e) => tracing::warn!(
                project = %self.projects[index].name,
                "Failed to spawn gitui PTY: {}", e
            ),
        }
    }

    /// Update running PTY programs when the theme changes.
    ///
    /// - **Neovim**: sends `:set background=dark|light` (preserves buffers).
    /// - **Shell**: exports updated env vars and re-renders the prompt
    ///   so colours reflect the new theme immediately.
    /// - **Render-time ANSI remap** in each pane widget handles indexed
    ///   colours that programs already emitted.
    pub fn update_ptys_for_theme(&mut self) {
        let is_dark = {
            if let ratatui::style::Color::Rgb(r, g, b) = self.theme.background {
                let lum = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
                lum < 128.0
            } else {
                true
            }
        };

        let bg = if is_dark { "dark" } else { "light" };
        let colorscheme_path = theme_gen::theme_dir().join("nvim/colors/opencode.lua");
        let nvim_cmd = format!(
            "\x1b:set background={} | luafile {}\r",
            bg,
            colorscheme_path.display()
        );

        let theme_dir = theme_gen::theme_dir();
        let zsh_theme = theme_dir.join("opencode.zsh");
        let shell_cmd = format!(" source '{}'; clear\n", zsh_theme.display());

        for project in self.projects.iter_mut() {
            for resources in project.session_resources.values_mut() {
                if let Some(ref mut nvim) = resources.neovim_pty {
                    let _ = nvim.write(nvim_cmd.as_bytes());
                }
                for shell in &mut resources.shell_ptys {
                    let _ = shell.write(shell_cmd.as_bytes());
                }
            }
            // Kill and respawn gitui_pty to pick up new theme
            if project.gitui_pty.is_some() {
                project.gitui_pty = None; // drop kills the process
                                          // Will be respawned by ensure_gitui_pty on next render
            }
        }
    }

    #[allow(dead_code)]
    pub fn neovim_terminal_inner_size(&self) -> (u16, u16) {
        if let Some(rect) = self.layout.panel_rect(PanelId::NeovimPane) {
            (rect.height, rect.width)
        } else {
            (0, 0)
        }
    }

    /// Handle a background event received via the mpsc channel.
    /// Called from the main event loop's `try_recv()` drain.
    pub fn handle_background_event(&mut self, event: BackgroundEvent) {
        match event {
            BackgroundEvent::PtySpawned {
                project_idx,
                session_id,
                pty,
            } => {
                if let Some(project) = self.projects.get_mut(project_idx) {
                    info!(name = %project.name, session_id, "PTY spawned via background event");
                    project.ptys.insert(session_id.clone(), pty);
                    project.active_session = Some(session_id);
                }
                self.resize_all_ptys();
            }
            BackgroundEvent::SessionsFetched {
                project_idx,
                sessions,
            } => {
                if let Some(project) = self.projects.get_mut(project_idx) {
                    let dir = project.path.to_string_lossy().to_string();
                    let filtered: Vec<SessionInfo> = sessions
                        .into_iter()
                        .filter(|s| s.directory == dir)
                        .collect();
                    for s in &filtered {
                        self.session_ownership.insert(s.id.clone(), project_idx);
                    }
                    project.sessions = filtered;
                }
            }
            BackgroundEvent::SessionFetchFailed { project_idx } => {
                debug!(project_idx, "Session fetch failed (non-fatal)");
            }
            BackgroundEvent::SessionSelected {
                project_idx,
                session_id,
            } => {
                debug!(
                    project_idx,
                    session_id, "Session selected via background event"
                );
            }
            BackgroundEvent::ProjectActivated { project_idx } => {
                debug!(project_idx, "Project fully activated via background event");
            }
            BackgroundEvent::SseSessionCreated {
                project_idx,
                session,
            } => {
                let awaiting = self.awaiting_new_session == Some(project_idx);

                if !awaiting {
                    if let Some(&owner) = self.session_ownership.get(&session.id) {
                        if owner != project_idx {
                            return;
                        }
                    }
                }

                if let Some(project) = self.projects.get_mut(project_idx) {
                    if !project.sessions.iter().any(|s| s.id == session.id) {
                        info!(name = %project.name, session_id = %session.id, "SSE: new session created");
                        self.active_sessions.insert(session.id.clone());
                        self.session_ownership
                            .insert(session.id.clone(), project_idx);
                        project.sessions.insert(0, session.clone());
                    }
                    if awaiting {
                        if let Some(pty) = project.ptys.remove("__new__") {
                            project.ptys.insert(session.id.clone(), pty);
                            project.active_session = Some(session.id.clone());
                        }
                        self.awaiting_new_session = None;
                        self.pending_session_select = Some((project_idx, session.id));
                    }
                }
            }
            BackgroundEvent::SseSessionUpdated {
                project_idx,
                session,
            } => {
                if let Some(&owner) = self.session_ownership.get(&session.id) {
                    if owner != project_idx {
                        return;
                    }
                }
                self.active_sessions.insert(session.id.clone());
                if let Some(project) = self.projects.get_mut(project_idx) {
                    if let Some(existing) = project.sessions.iter_mut().find(|s| s.id == session.id)
                    {
                        *existing = session;
                    }
                }
            }
            BackgroundEvent::SseSessionDeleted {
                project_idx,
                session_id,
            } => {
                self.active_sessions.remove(&session_id);
                self.session_ownership.remove(&session_id);
                if let Some(project) = self.projects.get_mut(project_idx) {
                    project.sessions.retain(|s| s.id != session_id);

                    // Kill and remove per-session resources (shell PTYs + neovim)
                    if let Some(mut resources) = project.session_resources.remove(&session_id) {
                        for shell_pty in &mut resources.shell_ptys {
                            let _ = shell_pty.kill();
                        }
                        if let Some(ref mut nvim) = resources.neovim_pty {
                            let _ = nvim.kill();
                        }
                    }

                    // Kill and remove the opencode PTY for this session
                    if let Some(mut pty) = project.ptys.remove(&session_id) {
                        let _ = pty.kill();
                    }

                    // If the deleted session was the active one, clear it
                    if project.active_session.as_deref() == Some(&session_id) {
                        project.active_session = None;
                    }
                }
            }
            BackgroundEvent::SseSessionIdle { session_id, .. } => {
                self.active_sessions.remove(&session_id);
            }
            BackgroundEvent::SseSessionBusy { session_id } => {
                self.active_sessions.insert(session_id);
            }
            BackgroundEvent::SseFileEdited {
                project_idx,
                file_path,
            } => {
                debug!(
                    project_idx,
                    file_path,
                    follow_enabled = self.config.settings.follow_edits_in_neovim,
                    neovim_mcp = self.neovim_mcp_enabled,
                    active_project = self.active_project,
                    "SseFileEdited received"
                );
                // When Neovim MCP is enabled, the AI edits files through Neovim
                // directly, so follow-edits is redundant and skipped.
                if !self.neovim_mcp_enabled
                    && self.config.settings.follow_edits_in_neovim
                    && project_idx == self.active_project
                {
                    let has_nvim = self
                        .projects
                        .get(project_idx)
                        .and_then(|p| p.active_resources())
                        .map(|r| r.neovim_pty.is_some())
                        .unwrap_or(false);
                    if !has_nvim {
                        self.ensure_neovim_pty();
                    }
                    if let Some(project) = self.projects.get_mut(project_idx) {
                        let project_path = project.path.clone();
                        if let Some(resources) = project.active_resources_mut() {
                            let has_nvim = resources.neovim_pty.is_some();
                            debug!(
                                has_nvim,
                                "SseFileEdited: project found, checking neovim_pty"
                            );
                            if let Some(ref mut nvim) = resources.neovim_pty {
                                // Build the absolute path for Neovim commands.
                                let abs_path = if std::path::Path::new(&file_path).is_absolute() {
                                    file_path.clone()
                                } else {
                                    project_path.join(&file_path).to_string_lossy().to_string()
                                };
                                let vim_str_path = abs_path.replace('\'', "''");

                                let mut cmds = vec![format!(
                                    "\x1b:execute 'edit! ' . fnameescape('{}')\r",
                                    vim_str_path
                                )];

                                let current_content =
                                    std::fs::read_to_string(&abs_path).unwrap_or_default();

                                let old_content =
                                    if let Some(snap) = resources.file_snapshots.get(&abs_path) {
                                        snap.clone()
                                    } else {
                                        let rel_path = std::path::Path::new(&abs_path)
                                            .strip_prefix(&project_path)
                                            .map(|p| p.to_string_lossy().to_string())
                                            .unwrap_or_else(|_| file_path.clone());
                                        let git_show = std::process::Command::new("git")
                                            .args(["show", &format!("HEAD:{}", rel_path)])
                                            .current_dir(&project_path)
                                            .output();
                                        match git_show {
                                            Ok(output) if output.status.success() => {
                                                String::from_utf8_lossy(&output.stdout).to_string()
                                            }
                                            _ => String::new(),
                                        }
                                    };

                                let (added, deleted) =
                                    if old_content.is_empty() && !current_content.is_empty() {
                                        let line_count = current_content.lines().count().max(1);
                                        ((1..=line_count).collect::<Vec<_>>(), Vec::new())
                                    } else {
                                        diff_snapshot_lines(&old_content, &current_content)
                                    };

                                resources
                                    .file_snapshots
                                    .insert(abs_path.clone(), current_content);

                                debug!(
                                    added_count = added.len(),
                                    deleted_count = deleted.len(),
                                    "SseFileEdited: snapshot diff computed"
                                );

                                if !added.is_empty() || !deleted.is_empty() {
                                    let success_hex = color_to_hex(self.theme.success);
                                    let error_hex = color_to_hex(self.theme.error);
                                    cmds.push(format!(
                                        "\x1b:highlight DiffAddLine guibg={} guifg=black\r",
                                        success_hex
                                    ));
                                    cmds.push(format!(
                                        "\x1b:highlight DiffDelLine guibg={} guifg=black\r",
                                        error_hex
                                    ));
                                    cmds.push(
                                        "\x1b:sign define diff_add text=+ texthl=DiffAddLine\r"
                                            .to_string(),
                                    );
                                    cmds.push(
                                        "\x1b:sign define diff_del text=- texthl=DiffDelLine\r"
                                            .to_string(),
                                    );
                                    cmds.push(
                                        "\x1b:execute 'sign unplace * buffer=' . bufnr('%')\r"
                                            .to_string(),
                                    );

                                    let mut sign_id = 1;
                                    let mut first_line: Option<usize> = None;
                                    for line in &added {
                                        first_line =
                                            Some(first_line.map_or(*line, |m: usize| m.min(*line)));
                                        cmds.push(format!(
                                            "\x1b:execute 'sign place {} line={} name=diff_add buffer=' . bufnr('%')\r",
                                            sign_id, line
                                        ));
                                        sign_id += 1;
                                    }
                                    for line in &deleted {
                                        first_line =
                                            Some(first_line.map_or(*line, |m: usize| m.min(*line)));
                                        cmds.push(format!(
                                            "\x1b:execute 'sign place {} line={} name=diff_del buffer=' . bufnr('%')\r",
                                            sign_id, line
                                        ));
                                        sign_id += 1;
                                    }

                                    if let Some(l) = first_line {
                                        cmds.push(format!("\x1b:call cursor({}, 0)\r", l));
                                        cmds.push("\x1b:normal! zz\r".to_string());
                                    }
                                }

                                let batch = cmds.concat();
                                debug!(
                                    cmd_count = cmds.len(),
                                    "SseFileEdited: writing batched vim cmds"
                                );
                                let _ = nvim.write(batch.as_bytes());
                            } else {
                                debug!(
                                    "SseFileEdited: neovim_pty is still None after ensure, skipping"
                                );
                            }
                        } else {
                            debug!("SseFileEdited: no active session resources");
                        }
                    } else {
                        debug!(project_idx, "SseFileEdited: project not found at index");
                    }
                } else {
                    debug!(
                        follow_enabled = self.config.settings.follow_edits_in_neovim,
                        project_match = (project_idx == self.active_project),
                        "SseFileEdited: skipped (follow disabled or wrong project)"
                    );
                }
            }
            BackgroundEvent::TodosFetched { session_id, todos } => {
                debug!(session_id, count = todos.len(), "Todos fetched");
                if let Some(ref mut panel) = self.todo_panel {
                    if panel.session_id == session_id {
                        panel.todos = todos;
                        if panel.selected >= panel.todos.len() {
                            panel.selected = panel.todos.len().saturating_sub(1);
                        }
                    }
                }
            }
            BackgroundEvent::SseTodoUpdated { session_id, todos } => {
                debug!(session_id, count = todos.len(), "SSE todo.updated");
                if let Some(ref mut panel) = self.todo_panel {
                    if panel.session_id == session_id {
                        panel.todos = todos;
                        if panel.selected >= panel.todos.len() {
                            panel.selected = panel.todos.len().saturating_sub(1);
                        }
                    }
                }
            }
            BackgroundEvent::SseMessageUpdated {
                session_id,
                cost,
                input_tokens,
                output_tokens,
                reasoning_tokens,
                cache_read,
                cache_write,
            } => {
                let stats = self
                    .session_stats
                    .entry(session_id.clone())
                    .or_insert_with(SessionStats::default);
                stats.cost = cost;
                stats.input_tokens = input_tokens;
                stats.output_tokens = output_tokens;
                stats.reasoning_tokens = reasoning_tokens;
                stats.cache_read = cache_read;
                stats.cache_write = cache_write;
                debug!(
                    session_id,
                    cost, input_tokens, output_tokens, "SSE: message.updated with token/cost data"
                );
            }
            BackgroundEvent::ModelLimitsFetched {
                project_idx,
                context_window,
            } => {
                self.model_limits
                    .insert(project_idx, ModelLimits { context_window });
                debug!(project_idx, context_window, "Model context window fetched");
            }
            BackgroundEvent::McpSocketRequest {
                project_idx,
                session_id,
                pending,
            } => {
                // If session_id is empty (e.g. no OPENCODE_SESSION_ID env var),
                // fall back to the project's active_session.
                let resolved_sid = if session_id.is_empty() {
                    self.projects
                        .get(project_idx)
                        .and_then(|p| p.active_session.clone())
                        .unwrap_or_default()
                } else {
                    session_id
                };
                let response =
                    self.handle_mcp_request(project_idx, &resolved_sid, &pending.request);
                let _ = pending.reply_tx.send(response);
            }
        }
    }

    /// Handle an MCP terminal tool request and return a SocketResponse.
    fn handle_mcp_request(
        &mut self,
        project_idx: usize,
        session_id: &str,
        request: &crate::mcp::SocketRequest,
    ) -> crate::mcp::SocketResponse {
        use crate::mcp::{SocketResponse, TabInfo};

        // Collect spawn parameters before borrowing project mutably.
        let shell_size = self
            .layout
            .panel_rect(crate::ui::layout_manager::PanelId::IntegratedTerminal)
            .map(|r| (r.height.saturating_sub(1).max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let nvim_size = self
            .layout
            .panel_rect(crate::ui::layout_manager::PanelId::NeovimPane)
            .map(|r| (r.height.max(2), r.width.max(2)))
            .unwrap_or((24, 80));
        let theme_envs = self.theme.pty_env_vars();
        let td = crate::theme_gen::theme_dir();
        let terminal_command = self
            .config
            .projects
            .get(project_idx)
            .and_then(|e| e.terminal_command.as_deref())
            .or(self.config.settings.default_terminal_command.as_deref())
            .map(|s| s.to_string());

        let project = match self.projects.get_mut(project_idx) {
            Some(p) => p,
            None => return SocketResponse::err("Project not found".into()),
        };
        let project_path = project.path.clone();
        let resources = project
            .session_resources
            .entry(session_id.to_string())
            .or_insert_with(SessionResources::new);

        // Determine if this is a terminal op that needs at least one shell tab.
        let needs_shell = matches!(
            request.op.as_str(),
            "read" | "run" | "close" | "rename" | "status"
        );
        // Determine if this is a neovim op.
        let needs_neovim = request.op.starts_with("nvim_");

        // Lazily spawn a shell PTY if needed and none exist.
        if needs_shell && resources.shell_ptys.is_empty() {
            match PtyInstance::spawn_shell(
                shell_size.0,
                shell_size.1,
                &project_path,
                &theme_envs,
                Some(&td),
                terminal_command.as_deref(),
                None,
            ) {
                Ok(shell) => {
                    resources.shell_ptys.push(shell);
                    resources.active_shell_tab = 0;
                }
                Err(e) => {
                    return SocketResponse::err(format!("Failed to auto-start terminal: {}", e));
                }
            }
        }

        // Lazily spawn neovim if needed and not running.
        if needs_neovim && resources.neovim_pty.is_none() {
            match PtyInstance::spawn_neovim(
                nvim_size.0,
                nvim_size.1,
                &project_path,
                &theme_envs,
                Some(&td),
            ) {
                Ok(nvim) => {
                    resources.neovim_pty = Some(nvim);
                }
                Err(e) => {
                    return SocketResponse::err(format!("Failed to auto-start neovim: {}", e));
                }
            }
        }

        match request.op.as_str() {
            "read" => {
                let tab_idx = request.tab.unwrap_or(resources.active_shell_tab);
                match resources.shell_ptys.get(tab_idx) {
                    Some(pty) => {
                        if let Ok(parser) = pty.parser.lock() {
                            let screen = parser.screen();
                            let (rows, cols) = screen.size();
                            let text = if let (Some(from), Some(to)) =
                                (request.from_line, request.to_line)
                            {
                                // Range read: from_line..=to_line (0-based)
                                let from = (from as u16).min(rows.saturating_sub(1));
                                let to = (to as u16).min(rows.saturating_sub(1));
                                screen.contents_between(from, 0, to + 1, cols)
                            } else if let Some(n) = request.last_n {
                                // Last N lines
                                let total = rows as usize;
                                let start = total.saturating_sub(n) as u16;
                                screen.contents_between(start, 0, rows, cols)
                            } else {
                                // Full screen contents (default)
                                screen.contents()
                            };
                            SocketResponse::ok_text(text)
                        } else {
                            SocketResponse::err("Failed to lock terminal parser".into())
                        }
                    }
                    None => SocketResponse::err(format!("Tab {} not found", tab_idx)),
                }
            }
            "run" => {
                let tab_idx = request.tab.unwrap_or(resources.active_shell_tab);
                let command = match &request.command {
                    Some(c) => c,
                    None => return SocketResponse::err("Missing 'command' for run op".into()),
                };
                match resources.shell_ptys.get_mut(tab_idx) {
                    Some(pty) => {
                        let is_ctrl_c = command == "\x03";
                        if !is_ctrl_c {
                            if let Ok(state) = pty.command_state.lock() {
                                if *state == crate::pty::CommandState::Running {
                                    return SocketResponse::err(
                                        "A command is already running on this tab. Send Ctrl-C (\\x03) to interrupt it first.".into()
                                    );
                                }
                            }
                        }
                        let bytes = if is_ctrl_c {
                            command.as_bytes().to_vec()
                        } else {
                            format!("{}\n", command).into_bytes()
                        };
                        match pty.write(&bytes) {
                            Ok(_) => {
                                SocketResponse::ok_text(format!("Command sent to tab {}", tab_idx))
                            }
                            Err(e) => {
                                SocketResponse::err(format!("Failed to write to terminal: {}", e))
                            }
                        }
                    }
                    None => SocketResponse::err(format!("Tab {} not found", tab_idx)),
                }
            }
            "list" => {
                let tabs: Vec<TabInfo> = resources
                    .shell_ptys
                    .iter()
                    .enumerate()
                    .map(|(i, pty)| TabInfo {
                        index: i,
                        active: i == resources.active_shell_tab,
                        name: if pty.name.is_empty() {
                            format!("Tab {}", i + 1)
                        } else {
                            pty.name.clone()
                        },
                    })
                    .collect();
                SocketResponse::ok_tabs(tabs)
            }
            "new" => {
                match crate::pty::PtyInstance::spawn_shell(
                    shell_size.0,
                    shell_size.1,
                    &project_path,
                    &theme_envs,
                    Some(&td),
                    terminal_command.as_deref(),
                    request.name.clone(),
                ) {
                    Ok(shell) => {
                        resources.shell_ptys.push(shell);
                        let new_idx = resources.shell_ptys.len() - 1;
                        resources.active_shell_tab = new_idx;
                        SocketResponse::ok_tab_created(new_idx)
                    }
                    Err(e) => SocketResponse::err(format!("Failed to spawn shell: {}", e)),
                }
            }
            "close" => {
                let tab_idx = request.tab.unwrap_or(resources.active_shell_tab);
                if tab_idx >= resources.shell_ptys.len() {
                    return SocketResponse::err(format!("Tab {} not found", tab_idx));
                }
                if resources.shell_ptys.len() <= 1 {
                    return SocketResponse::err("Cannot close the last tab".into());
                }
                let mut pty = resources.shell_ptys.remove(tab_idx);
                let _ = pty.kill();
                if resources.active_shell_tab >= resources.shell_ptys.len() {
                    resources.active_shell_tab = resources.shell_ptys.len().saturating_sub(1);
                }
                SocketResponse::ok_empty()
            }
            "rename" => {
                let tab_idx = match request.tab {
                    Some(idx) => idx,
                    None => return SocketResponse::err("Missing 'tab' for rename op".into()),
                };
                let name = match &request.name {
                    Some(n) => n,
                    None => return SocketResponse::err("Missing 'name' for rename op".into()),
                };
                match resources.shell_ptys.get_mut(tab_idx) {
                    Some(pty) => {
                        pty.name = name.clone();
                        SocketResponse::ok_empty()
                    }
                    None => SocketResponse::err(format!("Tab {} not found", tab_idx)),
                }
            }
            "status" => {
                let tab_idx = request.tab.unwrap_or(resources.active_shell_tab);
                match resources.shell_ptys.get(tab_idx) {
                    Some(pty) => {
                        let state = if let Ok(cs) = pty.command_state.lock() {
                            match *cs {
                                crate::pty::CommandState::Idle => "idle",
                                crate::pty::CommandState::Running => "running",
                                crate::pty::CommandState::Success => "success",
                                crate::pty::CommandState::Failure => "failure",
                            }
                        } else {
                            "unknown"
                        };
                        SocketResponse::ok_status(state.to_string())
                    }
                    None => SocketResponse::err(format!("Tab {} not found", tab_idx)),
                }
            }
            // ── Neovim operations ─────────────────────────────────────
            "nvim_open" | "nvim_read" | "nvim_command" | "nvim_buffers" | "nvim_info"
            | "nvim_diagnostics" | "nvim_definition" | "nvim_references" | "nvim_hover"
            | "nvim_symbols" | "nvim_code_actions" | "nvim_eval" | "nvim_grep" | "nvim_diff"
            | "nvim_write" | "nvim_edit" | "nvim_undo" | "nvim_rename" | "nvim_format"
            | "nvim_signature" => {
                let nvim_socket = match &resources.neovim_pty {
                    Some(pty) => match &pty.nvim_listen_addr {
                        Some(addr) => addr.clone(),
                        None => return SocketResponse::err(
                            "Neovim PTY has no listen address".into()
                        ),
                    },
                    None => return SocketResponse::err(
                        "Neovim is not running for this project. Focus the Neovim pane to start it.".into()
                    ),
                };

                match request.op.as_str() {
                    "nvim_open" => {
                        let file_path = match &request.file_path {
                            Some(p) => p.as_str(),
                            None => {
                                return SocketResponse::err(
                                    "Missing 'file_path' for nvim_open".into(),
                                )
                            }
                        };
                        match crate::nvim_rpc::nvim_open_file(&nvim_socket, file_path, request.line)
                        {
                            Ok(()) => {
                                let mut msg = format!("Opened {}", file_path);
                                if let Some(ln) = request.line {
                                    msg.push_str(&format!(" at line {}", ln));
                                }
                                SocketResponse::ok_text(msg)
                            }
                            Err(e) => {
                                SocketResponse::err(format!("Failed to open file in Neovim: {}", e))
                            }
                        }
                    }
                    "nvim_read" => {
                        // Convert from 1-indexed (user-facing) to 0-indexed (nvim API)
                        let start = request.line.unwrap_or(1).max(1) - 1;
                        let end = match request.end_line {
                            Some(-1) | None => {
                                // Read to end of buffer
                                match crate::nvim_rpc::nvim_buf_line_count(&nvim_socket) {
                                    Ok(count) => count,
                                    Err(e) => {
                                        return SocketResponse::err(format!(
                                            "Failed to get line count: {}",
                                            e
                                        ))
                                    }
                                }
                            }
                            Some(e) => e, // Already 1-indexed end, used as exclusive end
                        };
                        match crate::nvim_rpc::nvim_buf_get_lines(&nvim_socket, start, end) {
                            Ok(lines) => {
                                // Number the lines for readability (1-indexed)
                                let numbered: Vec<String> = lines
                                    .iter()
                                    .enumerate()
                                    .map(|(i, l)| format!("{}: {}", start + 1 + i as i64, l))
                                    .collect();
                                SocketResponse::ok_text(numbered.join("\n"))
                            }
                            Err(e) => SocketResponse::err(format!(
                                "Failed to read lines from Neovim: {}",
                                e
                            )),
                        }
                    }
                    "nvim_command" => {
                        let cmd = match &request.command {
                            Some(c) => c.as_str(),
                            None => {
                                return SocketResponse::err(
                                    "Missing 'command' for nvim_command".into(),
                                )
                            }
                        };
                        match crate::nvim_rpc::nvim_command(&nvim_socket, cmd) {
                            Ok(()) => SocketResponse::ok_text(format!("Command executed: {}", cmd)),
                            Err(e) => SocketResponse::err(format!("Neovim command failed: {}", e)),
                        }
                    }
                    "nvim_buffers" => match crate::nvim_rpc::nvim_list_bufs(&nvim_socket) {
                        Ok(bufs) => {
                            if bufs.is_empty() {
                                SocketResponse::ok_text("No named buffers loaded.".into())
                            } else {
                                let lines: Vec<String> = bufs
                                    .iter()
                                    .map(|(id, name)| format!("Buffer {}: {}", id, name))
                                    .collect();
                                SocketResponse::ok_text(lines.join("\n"))
                            }
                        }
                        Err(e) => SocketResponse::err(format!("Failed to list buffers: {}", e)),
                    },
                    "nvim_info" => {
                        let name = crate::nvim_rpc::nvim_buf_get_name(&nvim_socket)
                            .unwrap_or_else(|_| "(unknown)".into());
                        let cursor =
                            crate::nvim_rpc::nvim_cursor_pos(&nvim_socket).unwrap_or((1, 0));
                        let line_count =
                            crate::nvim_rpc::nvim_buf_line_count(&nvim_socket).unwrap_or(0);

                        let info = format!(
                            "Buffer: {}\nCursor: line {}, column {}\nTotal lines: {}",
                            if name.is_empty() { "(unnamed)" } else { &name },
                            cursor.0,
                            cursor.1,
                            line_count
                        );
                        SocketResponse::ok_text(info)
                    }
                    "nvim_diagnostics" => {
                        let buf_only = request.buf_only.unwrap_or(false);
                        match crate::nvim_rpc::nvim_lsp_diagnostics(&nvim_socket, buf_only) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => {
                                SocketResponse::err(format!("Failed to get diagnostics: {}", e))
                            }
                        }
                    }
                    "nvim_definition" => {
                        match crate::nvim_rpc::nvim_lsp_definition(
                            &nvim_socket,
                            request.line,
                            request.col,
                        ) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => {
                                SocketResponse::err(format!("Failed to get definition: {}", e))
                            }
                        }
                    }
                    "nvim_references" => {
                        match crate::nvim_rpc::nvim_lsp_references(
                            &nvim_socket,
                            request.line,
                            request.col,
                        ) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => {
                                SocketResponse::err(format!("Failed to get references: {}", e))
                            }
                        }
                    }
                    "nvim_hover" => {
                        match crate::nvim_rpc::nvim_lsp_hover(
                            &nvim_socket,
                            request.line,
                            request.col,
                        ) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => {
                                SocketResponse::err(format!("Failed to get hover info: {}", e))
                            }
                        }
                    }
                    "nvim_symbols" => {
                        let query = request.query.as_deref().unwrap_or("");
                        let workspace = request.workspace.unwrap_or(false);
                        match crate::nvim_rpc::nvim_lsp_symbols(&nvim_socket, query, workspace) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => SocketResponse::err(format!("Failed to get symbols: {}", e)),
                        }
                    }
                    "nvim_code_actions" => {
                        match crate::nvim_rpc::nvim_lsp_code_actions(&nvim_socket) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => {
                                SocketResponse::err(format!("Failed to get code actions: {}", e))
                            }
                        }
                    }
                    "nvim_eval" => {
                        let code = match &request.command {
                            Some(c) => c.as_str(),
                            None => {
                                return SocketResponse::err(
                                    "Missing 'command' (Lua code) for nvim_eval".into(),
                                )
                            }
                        };
                        match crate::nvim_rpc::nvim_eval_lua(&nvim_socket, code) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => SocketResponse::err(format!("Lua eval failed: {}", e)),
                        }
                    }
                    "nvim_grep" => {
                        let pattern = match &request.query {
                            Some(q) => q.as_str(),
                            None => {
                                return SocketResponse::err(
                                    "Missing 'query' (search pattern) for nvim_grep".into(),
                                )
                            }
                        };
                        let glob = request.glob.as_deref();
                        match crate::nvim_rpc::nvim_grep(&nvim_socket, pattern, glob) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => SocketResponse::err(format!("Grep failed: {}", e)),
                        }
                    }
                    "nvim_diff" => match crate::nvim_rpc::nvim_buf_diff(&nvim_socket) {
                        Ok(output) => {
                            if output.is_empty() {
                                SocketResponse::ok_text("No unsaved changes.".into())
                            } else {
                                SocketResponse::ok_text(output)
                            }
                        }
                        Err(e) => SocketResponse::err(format!("Failed to compute diff: {}", e)),
                    },
                    "nvim_write" => {
                        let all = request.all.unwrap_or(false);
                        match crate::nvim_rpc::nvim_write(&nvim_socket, all) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => SocketResponse::err(format!("Failed to write: {}", e)),
                        }
                    }
                    // ── Editing ──────────────────────────────────────
                    "nvim_edit" => {
                        let start_line = match request.line {
                            Some(l) => l,
                            None => {
                                return SocketResponse::err(
                                    "Missing 'start_line' for nvim_edit".into(),
                                )
                            }
                        };
                        let end_line = match request.end_line {
                            Some(l) => l,
                            None => {
                                return SocketResponse::err(
                                    "Missing 'end_line' for nvim_edit".into(),
                                )
                            }
                        };
                        let new_text = match &request.new_text {
                            Some(t) => t.as_str(),
                            None => {
                                return SocketResponse::err(
                                    "Missing 'new_text' for nvim_edit".into(),
                                )
                            }
                        };
                        match crate::nvim_rpc::nvim_buf_set_text(
                            &nvim_socket,
                            start_line,
                            end_line,
                            new_text,
                        ) {
                            Ok(msg) => SocketResponse::ok_text(msg),
                            Err(e) => SocketResponse::err(format!("Edit failed: {}", e)),
                        }
                    }
                    "nvim_undo" => {
                        let count = request.count.unwrap_or(1);
                        match crate::nvim_rpc::nvim_undo(&nvim_socket, count) {
                            Ok(msg) => SocketResponse::ok_text(msg),
                            Err(e) => SocketResponse::err(format!("Undo failed: {}", e)),
                        }
                    }
                    // ── LSP Refactoring ──────────────────────────────
                    "nvim_rename" => {
                        let new_name = match &request.new_name {
                            Some(n) => n.as_str(),
                            None => {
                                return SocketResponse::err(
                                    "Missing 'new_name' for nvim_rename".into(),
                                )
                            }
                        };
                        match crate::nvim_rpc::nvim_lsp_rename(
                            &nvim_socket,
                            new_name,
                            request.line,
                            request.col,
                        ) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => SocketResponse::err(format!("Rename failed: {}", e)),
                        }
                    }
                    "nvim_format" => match crate::nvim_rpc::nvim_lsp_format(&nvim_socket) {
                        Ok(output) => SocketResponse::ok_text(output),
                        Err(e) => SocketResponse::err(format!("Format failed: {}", e)),
                    },
                    "nvim_signature" => {
                        match crate::nvim_rpc::nvim_lsp_signature(
                            &nvim_socket,
                            request.line,
                            request.col,
                        ) {
                            Ok(output) => SocketResponse::ok_text(output),
                            Err(e) => SocketResponse::err(format!("Signature help failed: {}", e)),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            other => SocketResponse::err(format!("Unknown operation: {}", other)),
        }
    }

    // ── Session management ───────────────────────────────────────

    /// Get the sessions to display in the sidebar for a project (max 5 latest + pinned).
    /// Only returns parent sessions (parent_id is empty).
    /// Only returns sessions if this project is the one currently expanded.
    pub fn visible_sessions(&self, project_idx: usize) -> Vec<&SessionInfo> {
        // Only one project can have sessions expanded at a time
        if self.sessions_expanded_for != Some(project_idx) {
            return Vec::new();
        }

        let project = match self.projects.get(project_idx) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let pinned = self.pinned_sessions.get(&project_idx);
        let mut visible: Vec<&SessionInfo> = Vec::new();

        // Always show pinned sessions first (only parent sessions)
        if let Some(pinned_ids) = pinned {
            for pid in pinned_ids {
                if let Some(s) = project
                    .sessions
                    .iter()
                    .find(|s| &s.id == pid && s.parent_id.is_empty())
                {
                    visible.push(s);
                }
            }
        }

        // Then latest parent sessions (up to 5 total, excluding already-visible pinned ones)
        for session in project.sessions.iter() {
            if visible.len() >= 5 {
                break;
            }
            if !session.parent_id.is_empty() {
                continue;
            }
            if !visible.iter().any(|v| v.id == session.id) {
                visible.push(session);
            }
        }
        visible
    }

    /// Get subagent sessions for a given parent session ID within a project.
    pub fn subagent_sessions(
        &self,
        project_idx: usize,
        parent_session_id: &str,
    ) -> Vec<&SessionInfo> {
        let project = match self.projects.get(project_idx) {
            Some(p) => p,
            None => return Vec::new(),
        };
        project
            .sessions
            .iter()
            .filter(|s| s.parent_id == parent_session_id)
            .collect()
    }

    /// Whether a project has more parent sessions than what's visible.
    pub fn has_more_sessions(&self, project_idx: usize) -> bool {
        if self.sessions_expanded_for != Some(project_idx) {
            return false;
        }
        self.projects
            .get(project_idx)
            .map(|p| p.sessions.iter().filter(|s| s.parent_id.is_empty()).count() > 5)
            .unwrap_or(false)
    }

    /// Map a flat sidebar_selection index to the item it represents.
    pub fn sidebar_item_at(&self, selection: usize) -> Option<SidebarItem> {
        let mut idx = 0;
        for (i, _project) in self.projects.iter().enumerate() {
            if idx == selection {
                return Some(SidebarItem::Project(i));
            }
            idx += 1;

            // "New Session" item appears when sessions are expanded
            if self.sessions_expanded_for == Some(i) {
                if idx == selection {
                    return Some(SidebarItem::NewSession(i));
                }
                idx += 1;
            }

            let visible = self.visible_sessions(i);
            for session in &visible {
                if idx == selection {
                    return Some(SidebarItem::Session(i, session.id.clone()));
                }
                idx += 1;

                // Subagent sessions under this parent
                if self.subagents_expanded_for.as_deref() == Some(&session.id) {
                    let subs = self.subagent_sessions(i, &session.id);
                    for sub in &subs {
                        if idx == selection {
                            return Some(SidebarItem::SubAgentSession(i, sub.id.clone()));
                        }
                        idx += 1;
                    }
                }
            }

            if self.has_more_sessions(i) {
                if idx == selection {
                    return Some(SidebarItem::MoreSessions(i));
                }
                idx += 1;
            }
        }
        if idx == selection {
            return Some(SidebarItem::AddProject);
        }
        None
    }

    /// Total number of items in the sidebar (for navigation bounds).
    pub fn sidebar_item_count(&self) -> usize {
        let mut count = 0;
        for (i, _) in self.projects.iter().enumerate() {
            count += 1; // project
            if self.sessions_expanded_for == Some(i) {
                count += 1; // "New Session"
            }
            let vis = self.visible_sessions(i);
            for session in &vis {
                count += 1; // session
                if self.subagents_expanded_for.as_deref() == Some(&session.id) {
                    count += self.subagent_sessions(i, &session.id).len();
                }
            }
            if self.has_more_sessions(i) {
                count += 1; // "more..."
            }
        }
        count += 1; // "[+ Add]"
        count
    }

    /// Compute the flat sidebar index for a given project + session ID.
    /// Returns `None` if the session is not currently visible in the sidebar.
    fn sidebar_index_for_session(&self, project_idx: usize, session_id: &str) -> Option<usize> {
        let mut idx = 0;
        for (i, _) in self.projects.iter().enumerate() {
            idx += 1; // project row

            if self.sessions_expanded_for == Some(i) {
                idx += 1; // "New Session"
            }

            let vis = self.visible_sessions(i);
            for session in &vis {
                if i == project_idx && session.id == session_id {
                    return Some(idx);
                }
                idx += 1;

                if self.subagents_expanded_for.as_deref() == Some(&session.id) {
                    idx += self.subagent_sessions(i, &session.id).len();
                }
            }

            if self.has_more_sessions(i) {
                idx += 1;
            }
        }
        None
    }

    /// Keep `sidebar_selection` in sync with the active project's active
    /// session so the highlight always reflects what is shown in the
    /// terminal pane.
    pub fn sync_sidebar_to_active_session(&mut self) {
        let proj = self.active_project;
        if let Some(ref sid) = self
            .projects
            .get(proj)
            .and_then(|p| p.active_session.clone())
        {
            if let Some(flat) = self.sidebar_index_for_session(proj, sid) {
                self.sidebar_selection = flat;
            }
        }
    }

    /// Open session search mode for the active project.
    pub fn open_session_search(&mut self) {
        if let Some(project) = self.projects.get(self.active_project) {
            self.session_search_all = project.sessions.clone();
            self.session_search_results = self.session_search_all.clone();
        }
        self.session_search_mode = true;
        self.session_search_buffer.clear();
        self.session_search_cursor = 0;
        self.session_search_selected = 0;
    }

    /// Open the cross-project session selector overlay.
    /// Collects ALL sessions from ALL projects, sorted by time.updated descending.
    pub fn open_session_selector(&mut self) {
        let mut entries: Vec<SessionSelectorEntry> = Vec::new();
        for (idx, project) in self.projects.iter().enumerate() {
            for session in &project.sessions {
                entries.push(SessionSelectorEntry {
                    project_name: project.name.clone(),
                    project_idx: idx,
                    session: session.clone(),
                });
            }
        }
        // Sort by time.updated descending (most recently updated first)
        entries.sort_by(|a, b| b.session.time.updated.cmp(&a.session.time.updated));
        let filtered: Vec<usize> = (0..entries.len()).collect();
        self.session_selector = Some(SessionSelectorState {
            entries,
            query: String::new(),
            cursor_pos: 0,
            selected: 0,
            scroll_offset: 0,
            filtered,
        });
    }

    /// Close session search mode.
    pub fn close_session_search(&mut self) {
        self.session_search_mode = false;
        self.session_search_buffer.clear();
        self.session_search_cursor = 0;
        self.session_search_all.clear();
        self.session_search_results.clear();
        self.session_search_selected = 0;
    }

    /// Update search results based on current buffer (fuzzy match on title/id).
    pub fn update_session_search(&mut self) {
        let query = self.session_search_buffer.to_lowercase();
        if query.is_empty() {
            self.session_search_results = self.session_search_all.clone();
        } else {
            self.session_search_results = self
                .session_search_all
                .iter()
                .filter(|s| {
                    s.title.to_lowercase().contains(&query) || s.id.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
        }
        self.session_search_selected = 0;
    }

    /// Pin the currently selected search result so it shows in sidebar, return its ID.
    pub fn pin_selected_session(&mut self) -> Option<String> {
        let session = self
            .session_search_results
            .get(self.session_search_selected)?
            .clone();
        let entry = self.pinned_sessions.entry(self.active_project).or_default();
        if !entry.contains(&session.id) {
            entry.push(session.id.clone());
        }
        self.close_session_search();
        Some(session.id)
    }

    /// Expand `~` to the user's home directory in the input buffer.
    fn expand_tilde(&self, input: &str) -> String {
        if input.starts_with('~') {
            let home = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .to_string_lossy()
                .to_string();
            format!("{}{}", home, &input[1..])
        } else {
            input.to_string()
        }
    }

    /// Scan the filesystem and update the completions list based on current input.
    pub fn update_completions(&mut self) {
        self.completions.clear();
        self.completion_selected = 0;

        let input = self.input_buffer.clone();
        if input.is_empty() {
            self.completions_visible = false;
            return;
        }

        let expanded = self.expand_tilde(&input);
        let path = Path::new(&expanded);

        let (parent, prefix) = if expanded.ends_with('/') {
            (path.to_path_buf(), String::new())
        } else {
            let parent = path.parent().unwrap_or(Path::new("/")).to_path_buf();
            let prefix = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            (parent, prefix)
        };

        let show_hidden = prefix.starts_with('.');

        let entries = match std::fs::read_dir(&parent) {
            Ok(rd) => rd,
            Err(_) => {
                self.completions_visible = false;
                return;
            }
        };

        let mut matches: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|entry| {
                let ft = entry.file_type().ok();
                let is_dir = ft.map(|f| f.is_dir()).unwrap_or(false);
                if !is_dir {
                    return false;
                }
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') && !show_hidden {
                    return false;
                }
                name_str.to_lowercase().starts_with(&prefix.to_lowercase())
            })
            .map(|entry| {
                let full = parent.join(entry.file_name());
                full.to_string_lossy().to_string()
            })
            .collect();

        matches.sort();

        // Convert back: if user typed ~, keep ~ prefix in completions
        if input.starts_with('~') {
            let home = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/"))
                .to_string_lossy()
                .to_string();
            matches = matches
                .into_iter()
                .map(|m| {
                    if m.starts_with(&home) {
                        format!("~{}", &m[home.len()..])
                    } else {
                        m
                    }
                })
                .collect();
        }

        self.completions = matches;
        self.completions_visible = !self.completions.is_empty();
    }

    /// Apply the currently selected completion into the input buffer.
    pub fn apply_completion(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let idx = self.completion_selected.min(self.completions.len() - 1);
        let mut path = self.completions[idx].clone();
        if !path.ends_with('/') {
            path.push('/');
        }
        self.input_buffer = path;
        self.input_cursor = self.input_buffer.len();
        self.completions.clear();
        self.completion_selected = 0;
        self.completions_visible = false;
    }

    /// Complete the longest common prefix among all current completions.
    pub fn complete_common_prefix(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let first = &self.completions[0];
        let mut common_len = first.len();
        for c in &self.completions[1..] {
            common_len = common_len.min(
                first
                    .chars()
                    .zip(c.chars())
                    .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
                    .count(),
            );
        }
        let common: String = first.chars().take(common_len).collect();

        if common.len() > self.input_buffer.len() {
            self.input_buffer = common;
            self.input_cursor = self.input_buffer.len();
            // Re-scan to narrow down
            self.update_completions();
        }
    }

    /// Clear completion state.
    pub fn clear_completions(&mut self) {
        self.completions.clear();
        self.completion_selected = 0;
        self.completions_visible = false;
    }
}

/// Diff two file snapshots line-by-line using the `similar` crate and return
/// (added_lines, deleted_lines) as 1-based line numbers in the *new* file.
///
/// - `added` contains line numbers that are new or changed in the new file.
/// - `deleted` contains line numbers in the new file *after which* old lines were removed.
///   (If deletions occur at the very start, line 1 is used.)
fn diff_snapshot_lines(old: &str, new: &str) -> (Vec<usize>, Vec<usize>) {
    use similar::{ChangeTag, TextDiff};

    let diff = TextDiff::from_lines(old, new);
    let mut added = Vec::new();
    let mut deleted = Vec::new();

    // Track the current line number in the new file.
    let mut new_line: usize = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                new_line += 1;
            }
            ChangeTag::Insert => {
                new_line += 1;
                added.push(new_line);
            }
            ChangeTag::Delete => {
                // Mark deletion at the current position in the new file.
                // If nothing has been output yet, pin to line 1.
                deleted.push(new_line.max(1));
            }
        }
    }

    // Deduplicate deletion markers (multiple deletions at the same position).
    deleted.dedup();

    (added, deleted)
}

/// Parse `git diff --unified=0` output and return (added_lines, deleted_lines).
///
/// Hunk headers look like `@@ -old_start,old_count +new_start,new_count @@`.
/// Lines starting with `+` (not `+++`) are additions in the new file.
/// Lines starting with `-` (not `---`) are deletions (we mark them at the hunk start).
#[cfg(test)]
fn parse_unified_diff(diff: &str) -> (Vec<usize>, Vec<usize>) {
    let mut added = Vec::new();
    let mut deleted = Vec::new();

    for line in diff.lines() {
        if !line.starts_with("@@ ") {
            continue;
        }
        // Parse hunk header: @@ -old_start[,old_count] +new_start[,new_count] @@
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        // Parse the old range (-X,Y or -X)
        let old_part = parts[1].trim_start_matches('-');
        let old_count = if let Some((_start, count)) = old_part.split_once(',') {
            count.parse::<usize>().unwrap_or(0)
        } else {
            1 // no comma means exactly 1 line
        };

        // Parse the new range (+X,Y or +X)
        let new_part = parts[2].trim_start_matches('+');
        let (new_start, new_count) = if let Some((start, count)) = new_part.split_once(',') {
            (
                start.parse::<usize>().unwrap_or(1),
                count.parse::<usize>().unwrap_or(0),
            )
        } else {
            (new_part.parse::<usize>().unwrap_or(1), 1)
        };

        // Added lines in the new file
        for i in 0..new_count {
            added.push(new_start + i);
        }
        // Deletions: mark at new_start (the line where content was removed)
        if old_count > 0 && new_count == 0 {
            deleted.push(new_start.max(1));
        }
    }

    (added, deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_session(id: &str) -> SessionInfo {
        SessionInfo {
            id: id.to_string(),
            ..Default::default()
        }
    }

    /// Mirrors the SessionsFetched ownership recording from handle_background_event.
    /// The server is trusted to return correct per-project sessions, so no filtering
    /// is applied — ownership is recorded authoritatively via insert (overwrite).
    fn record_ownership(
        sessions: &[SessionInfo],
        project_idx: usize,
        ownership: &mut HashMap<String, usize>,
    ) {
        for s in sessions {
            ownership.insert(s.id.clone(), project_idx);
        }
    }

    /// Mirrors the SseSessionCreated/Updated ownership guard.
    fn is_owned_by_other(
        session_id: &str,
        project_idx: usize,
        ownership: &HashMap<String, usize>,
    ) -> bool {
        if let Some(&owner) = ownership.get(session_id) {
            return owner != project_idx;
        }
        false
    }

    #[test]
    fn test_sessions_fetched_records_ownership() {
        let mut ownership = HashMap::new();
        let sessions = vec![make_session("s1"), make_session("s2"), make_session("s3")];
        record_ownership(&sessions, 0, &mut ownership);

        assert_eq!(ownership.len(), 3);
        assert_eq!(ownership["s1"], 0);
        assert_eq!(ownership["s2"], 0);
        assert_eq!(ownership["s3"], 0);
    }

    #[test]
    fn test_sessions_fetched_overwrites_stale_ownership() {
        let mut ownership = HashMap::new();
        // s1 was previously claimed by project 0 via SSE
        ownership.insert("s1".to_string(), 0);

        // Server fetch for project 1 returns s1 (server knows s1 belongs to project 1)
        let sessions = vec![make_session("s1")];
        record_ownership(&sessions, 1, &mut ownership);

        // Ownership should be overwritten to project 1
        assert_eq!(ownership["s1"], 1);
    }

    #[test]
    fn test_sse_session_created_skips_if_owned_by_other() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);

        assert!(is_owned_by_other("s1", 1, &ownership));
        assert!(!is_owned_by_other("s1", 0, &ownership));
    }

    #[test]
    fn test_sse_session_created_claims_if_new() {
        let mut ownership = HashMap::new();
        assert!(!is_owned_by_other("s1", 1, &ownership));
        ownership.entry("s1".to_string()).or_insert(1);
        assert_eq!(ownership["s1"], 1);
    }

    #[test]
    fn test_sse_session_updated_skips_if_owned_by_other() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);

        // Project 1 should be rejected
        assert!(is_owned_by_other("s1", 1, &ownership));
        // Project 0 should be allowed
        assert!(!is_owned_by_other("s1", 0, &ownership));
        // Unknown session should be allowed
        assert!(!is_owned_by_other("unknown", 0, &ownership));
    }

    #[test]
    fn test_session_deleted_removes_ownership() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);
        ownership.remove("s1");
        assert!(!ownership.contains_key("s1"));
    }

    #[test]
    fn test_awaiting_session_overrides_ownership() {
        let mut ownership = HashMap::new();
        // s1 was incorrectly claimed by project 0 via SSE race
        ownership.insert("s1".to_string(), 0);

        // Project 1 has awaiting_new_session set (PTY spawned this session)
        // Force-claim with insert (overwrite), mirroring the handler logic
        ownership.insert("s1".to_string(), 1);
        assert_eq!(ownership["s1"], 1);
    }

    #[test]
    fn test_deleted_session_can_be_reclaimed() {
        let mut ownership = HashMap::new();
        ownership.insert("s1".to_string(), 0);
        ownership.remove("s1");

        // Project 1 can now claim it via SSE
        assert!(!is_owned_by_other("s1", 1, &ownership));
        ownership.entry("s1".to_string()).or_insert(1);
        assert_eq!(ownership["s1"], 1);
    }

    #[test]
    fn test_multiple_projects_independent_sessions() {
        let mut ownership = HashMap::new();

        // Project 0 fetches its sessions
        let p0_sessions = vec![make_session("s1"), make_session("s2")];
        record_ownership(&p0_sessions, 0, &mut ownership);

        // Project 1 fetches its sessions (different set)
        let p1_sessions = vec![make_session("s3"), make_session("s4")];
        record_ownership(&p1_sessions, 1, &mut ownership);

        assert_eq!(ownership["s1"], 0);
        assert_eq!(ownership["s2"], 0);
        assert_eq!(ownership["s3"], 1);
        assert_eq!(ownership["s4"], 1);

        // SSE for s3 on project 0 should be rejected
        assert!(is_owned_by_other("s3", 0, &ownership));
        // SSE for s1 on project 1 should be rejected
        assert!(is_owned_by_other("s1", 1, &ownership));
    }

    #[test]
    fn test_parse_unified_diff_additions() {
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdef0 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,0 +11,3 @@ fn main() {
+    let x = 1;
+    let y = 2;
+    let z = 3;
";
        let (added, deleted) = parse_unified_diff(diff);
        assert_eq!(added, vec![11, 12, 13]);
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_parse_unified_diff_deletions() {
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -5,2 +5,0 @@ fn main() {
-    old_line_1();
-    old_line_2();
";
        let (added, deleted) = parse_unified_diff(diff);
        assert!(added.is_empty());
        assert_eq!(deleted, vec![5]);
    }

    #[test]
    fn test_parse_unified_diff_mixed() {
        let diff = "\
@@ -3,2 +3,4 @@ fn foo() {
-    old1();
-    old2();
+    new1();
+    new2();
+    new3();
+    new4();
@@ -20,1 +22,0 @@ fn bar() {
-    removed();
";
        let (added, deleted) = parse_unified_diff(diff);
        assert_eq!(added, vec![3, 4, 5, 6]);
        // Second hunk: pure deletion at line 22
        assert_eq!(deleted, vec![22]);
    }

    #[test]
    fn test_parse_unified_diff_single_line() {
        let diff = "@@ -1 +1 @@\n-old\n+new\n";
        let (added, deleted) = parse_unified_diff(diff);
        assert_eq!(added, vec![1]);
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_snapshot_diff_additions() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline2\nnew_line\nline3\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        assert_eq!(added, vec![3]);
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_snapshot_diff_deletions() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline3\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        assert!(added.is_empty());
        assert_eq!(deleted, vec![1]); // deletion after line 1
    }

    #[test]
    fn test_snapshot_diff_mixed() {
        let old = "aaa\nbbb\nccc\n";
        let new = "aaa\nXXX\nccc\nYYY\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        // bbb replaced by XXX, YYY appended
        assert_eq!(added, vec![2, 4]);
        assert_eq!(deleted, vec![1]); // bbb deleted after line 1
    }

    #[test]
    fn test_snapshot_diff_empty_to_content() {
        let old = "";
        let new = "hello\nworld\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        assert_eq!(added, vec![1, 2]);
        assert!(deleted.is_empty());
    }

    #[test]
    fn test_snapshot_diff_no_change() {
        let old = "same\ncontent\n";
        let new = "same\ncontent\n";
        let (added, deleted) = diff_snapshot_lines(old, new);
        assert!(added.is_empty());
        assert!(deleted.is_empty());
    }
}
