// ── Sub-modules ─────────────────────────────────────────────────────
mod accessors;
mod background;
mod background_event;
mod background_sse;
mod background_sse_slack;
mod completions;
mod context_input;
pub mod helpers;
#[cfg(test)]
mod helpers_tests;
mod mcp_handler;
mod mcp_operations;
mod project;
mod pty_management;
mod session;
mod session_selector_types;
mod sidebar;
mod slack_actions;
mod slack_commands;
mod slack_connect_modal;
mod slack_dispatch;
mod slack_messages;
mod slack_modals;
mod slack_pending;
mod slack_questions;
mod slack_session;
mod slack_slash_triage;
mod slack_thread;
mod slack_triage;
mod slack_triage_result;
mod slack_triage_run;
mod slack_types;
mod types;
mod watcher;
mod watcher_types;

// ── Re-exports ──────────────────────────────────────────────────────
pub use background_event::BackgroundEvent;
pub use context_input::ContextInputState;
pub use helpers::{diff_snapshot_lines, read_full_terminal_buffer};
pub use session_selector_types::{ServerStatus, SessionSelectorEntry, SessionSelectorState};
pub use slack_types::PendingSlackMessage;
pub use types::*;
pub use watcher_types::*;

// ── Imports ─────────────────────────────────────────────────────────
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::command_palette::CommandPalette;
use crate::config::Config;
use crate::theme::ThemeColors;
use crate::ui::fuzzy_picker::FuzzyPickerState;
use crate::ui::layout_manager::{LayoutManager, PanelId};
use crate::vim_mode::{EscapeTracker, VimMode};
use crate::which_key::{RuntimeKeyBinding, WhichKeyState};

// ── App struct ──────────────────────────────────────────────────────
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
    pub sessions_expanded_for: Option<usize>,
    /// Set by input handler when user triggers "New Session".
    pub pending_new_session: Option<usize>,
    /// Persists until SSE session.created arrives for this project.
    pub awaiting_new_session: Option<usize>,
    /// Session IDs that are currently active/running (not idle).
    pub active_sessions: HashSet<String>,
    /// Session IDs that have encountered an error.
    pub error_sessions: HashSet<String>,
    /// Session IDs that need user input (pending permission or question).
    pub input_sessions: HashSet<String>,
    /// Session IDs with unseen activity (idle/error while not viewing).
    pub unseen_sessions: HashSet<String>,
    /// Sine-wave phase for pulsating active session dot.
    pub pulse_phase: f64,
    /// Which parent session ID has its subagent sessions expanded.
    pub subagents_expanded_for: Option<String>,
    pub vim_mode: VimMode,
    pub escape_tracker: EscapeTracker,
    pub command_palette: CommandPalette,
    pub which_key: WhichKeyState,
    pub runtime_keymap: Vec<RuntimeKeyBinding>,
    pub zen_mode: bool,
    pub pre_zen_state: Option<([bool; 5], PanelId)>,
    pub popout_mode: bool,
    pub pre_popout_state: Option<([bool; 5], PanelId)>,
    pub popout_windows: Vec<std::process::Child>,
    pub show_config_panel: bool,
    pub config_panel_selected: usize,
    pub show_slack_log: bool,
    pub slack_log_scroll: usize,
    pub session_selector: Option<SessionSelectorState>,
    pub todo_panel: Option<TodoPanelState>,
    pub routine_panel: Option<RoutinePanelState>,
    pub session_stats: HashMap<String, SessionStats>,
    pub model_limits: HashMap<usize, ModelLimits>,
    pub neovim_mcp_enabled: bool,
    pub bg_tx: mpsc::UnboundedSender<BackgroundEvent>,
    pub nvim_registry: crate::mcp::NvimSocketRegistry,
    pub toast_message: Option<(String, std::time::Instant)>,
    pub terminal_selection: Option<TerminalSelection>,
    pub terminal_search: Option<TerminalSearchState>,
    pub context_input: Option<ContextInputState>,
    pub session_watchers: HashMap<String, WatcherConfig>,
    pub watcher_modal: Option<WatcherModalState>,
    pub watcher_pending: HashMap<String, tokio::task::AbortHandle>,
    pub watcher_idle_since: HashMap<String, std::time::Instant>,
    pub session_ownership: HashMap<String, usize>,
    pub session_children: HashMap<String, HashSet<String>>,
    pub needs_redraw: bool,
    pub status_bar_url_range: std::cell::Cell<Option<(u16, u16)>>,
    pub last_mcp_activity_ms: Arc<std::sync::atomic::AtomicU64>,
    pub last_message_event_at: HashMap<String, std::time::Instant>,
    pub slack_state: Option<Arc<tokio::sync::Mutex<crate::slack::SlackState>>>,
    pub slack_auth: Option<crate::slack::SlackAuth>,
    pub pending_slack_messages: Vec<PendingSlackMessage>,
    /// Handle to the opman web state (routines, missions, etc.).
    /// `None` when the web server is disabled.  Used by the TUI routine
    /// panel to call routine methods directly (no HTTP round-trip needed).
    pub web_state: Option<crate::web::WebStateHandle>,
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
            error_sessions: HashSet::new(),
            input_sessions: HashSet::new(),
            unseen_sessions: HashSet::new(),
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
            show_slack_log: false,
            slack_log_scroll: 0,
            session_selector: None,
            todo_panel: None,
            routine_panel: None,
            session_stats: HashMap::new(),
            model_limits: HashMap::new(),
            neovim_mcp_enabled: false,
            bg_tx,
            nvim_registry: crate::mcp::new_nvim_socket_registry(),
            toast_message: None,
            terminal_selection: None,
            terminal_search: None,
            context_input: None,
            session_watchers: HashMap::new(),
            watcher_modal: None,
            watcher_pending: HashMap::new(),
            watcher_idle_since: HashMap::new(),
            session_ownership: HashMap::new(),
            session_children: HashMap::new(),
            needs_redraw: true,
            status_bar_url_range: std::cell::Cell::new(None),
            last_mcp_activity_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            last_message_event_at: HashMap::new(),
            slack_state: None,
            slack_auth: None,
            pending_slack_messages: Vec::new(),
            web_state: None,
        }
    }

    pub fn active_project(&self) -> Option<&Project> {
        self.projects.get(self.active_project)
    }

    pub fn active_project_mut(&mut self) -> Option<&mut Project> {
        self.projects.get_mut(self.active_project)
    }
}
