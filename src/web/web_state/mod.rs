//! Independent web state manager.
//!
//! This module provides a fully decoupled state layer for the web UI.
//! It talks directly to the `opencode serve` REST API and SSE stream,
//! maintaining its own:
//!
//! - Project list (loaded from `Config`)
//! - Sessions per project (polled from `GET /session`)
//! - Session stats (captured from opencode SSE `message.updated` events)
//! - Busy/idle session tracking (from SSE `session.status` events)
//! - Active project index, panel visibility, focused panel
//!
//! No TUI dependency — the web server is fully standalone.

mod assistant;
mod background;
mod db_sync;
mod delegation;
mod file_edits;
mod intelligence_inbox;
mod intelligence_recs;
mod intelligence_handoffs;
mod intelligence_misc;
mod mutations;
mod presence;
mod queries;
mod sse;
mod sse_handler;
mod watchers;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{broadcast, RwLock};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use tracing::info;

use crate::config::Config;

use super::db::Db;
use super::types::*;

// ── Internal state ──────────────────────────────────────────────────

/// Per-project data maintained by the web state manager.
#[derive(Debug, Clone)]
pub(super) struct WebProject {
    pub(super) name: String,
    pub(super) path: PathBuf,
    pub(super) sessions: Vec<crate::app::SessionInfo>,
    pub(super) active_session: Option<String>,
    pub(super) git_branch: String,
}

/// Inner mutable state protected by `RwLock`.
pub(super) struct WebStateInner {
    pub(super) projects: Vec<WebProject>,
    pub(super) active_project: usize,
    /// Panel visibility (sidebar, terminal_pane, neovim_pane, integrated_terminal, git_panel).
    pub(super) panels: WebPanelVisibility,
    /// Currently focused panel name.
    pub(super) focused: String,
    /// Per-session cost/token stats, keyed by session ID.
    pub(super) session_stats: HashMap<String, WebSessionStats>,
    /// Set of session IDs currently busy.
    pub(super) busy_sessions: HashSet<String>,
    /// Current theme colors (hex strings) for the web frontend.
    pub(super) theme: Option<WebThemeColors>,
    // ── Watcher state ────────────────────────────────────────────
    /// Active watcher configurations, keyed by session ID.
    pub(super) session_watchers: HashMap<String, WatcherConfigInternal>,
    /// Pending watcher timers (abort handles for delayed continuation sends).
    pub(super) watcher_pending: HashMap<String, AbortHandle>,
    /// When each watched session went idle (for countdown display).
    pub(super) watcher_idle_since: HashMap<String, Instant>,
    /// Parent→children mapping for subagent suppression.
    pub(super) session_children: HashMap<String, HashSet<String>>,
    // ── File edit tracking (diff review) ─────────────────────────
    /// Per-session file snapshots: session_id → (file_path → original_content).
    /// Stores the content of a file *before* the first edit in the session.
    pub(super) file_snapshots: HashMap<String, HashMap<String, String>>,
    /// Per-session ordered list of file edit events.
    pub(super) file_edits: HashMap<String, Vec<FileEditRecord>>,
    // ── Session Continuity: presence + activity ──────────────────
    /// Connected clients, keyed by client_id.
    pub(super) connected_clients: HashMap<String, ClientPresence>,
    /// Per-session recent activity events (ring buffer, max 200 per session).
    pub(super) activity_log: HashMap<String, Vec<ActivityEventPayload>>,
    /// Saved missions, keyed by ID.
    pub(super) missions: HashMap<String, Mission>,
    /// Saved personal memory items, keyed by ID.
    pub(super) personal_memory: HashMap<String, PersonalMemoryItem>,
    /// Current autonomy settings.
    pub(super) autonomy_settings: AutonomySettings,
    /// Saved routines.
    pub(super) routines: HashMap<String, RoutineDefinition>,
    /// Routine execution history.
    pub(super) routine_runs: Vec<RoutineRunRecord>,
    /// Delegated work items.
    pub(super) delegated_work: HashMap<String, DelegatedWorkItem>,
    // ── Workspace Snapshots ─────────────────────────────────────
    /// Saved workspace snapshots, keyed by name.
    pub(super) workspaces: HashMap<String, WorkspaceSnapshot>,
    // ── Signals ─────────────────────────────────────────────────
    /// Persisted assistant signals (newest first, max 100).
    pub(super) signals: Vec<SignalInput>,
    // ── Pending permissions & questions (ephemeral, from SSE) ──
    /// Pending permission requests, keyed by request ID.
    /// Stored as raw JSON so the frontend receives the same shape as SSE events.
    pub(super) pending_permissions: HashMap<String, serde_json::Value>,
    /// Pending question requests, keyed by request ID.
    pub(super) pending_questions: HashMap<String, serde_json::Value>,
}

/// Internal watcher config (stored on the server side).
#[derive(Clone, Debug)]
pub(super) struct WatcherConfigInternal {
    pub(super) session_id: String,
    pub(super) project_idx: usize,
    pub(super) idle_timeout_secs: u64,
    pub(super) continuation_message: String,
    pub(super) include_original: bool,
    pub(super) original_message: Option<String>,
    pub(super) hang_message: String,
    pub(super) hang_timeout_secs: u64,
}

/// Internal record for a single file edit event.
#[derive(Clone, Debug)]
pub(crate) struct FileEditRecord {
    /// File path (relative to project root, or absolute).
    pub(crate) path: String,
    /// Content before the edit.
    pub(crate) original_content: String,
    /// Content after the edit.
    pub(crate) new_content: String,
    /// When the edit was recorded.
    pub(crate) timestamp: String,
    /// Sequential index.
    pub(crate) index: usize,
}

// ── Public handle ───────────────────────────────────────────────────

/// Async-safe, cloneable handle to the web state. Used by Axum handlers.
#[derive(Clone)]
pub struct WebStateHandle {
    pub(super) inner: Arc<RwLock<WebStateInner>>,
    /// Broadcast channel for notifying SSE clients of state changes.
    pub(super) event_tx: broadcast::Sender<WebEvent>,
    /// Broadcast channel for raw upstream opencode SSE event data.
    /// The session_events_stream subscribes here to forward events to the browser.
    pub(super) raw_sse_tx: broadcast::Sender<String>,
    /// SQLite database handle (replaces JSON persistence).
    pub(super) db: Db,
    /// Channel to trigger async DB writes (debounced).
    pub(super) persist_tx: mpsc::UnboundedSender<()>,
}

impl WebStateHandle {
    /// Create the web state from config, start background pollers.
    ///
    /// `event_tx` is the broadcast channel that SSE clients subscribe to.
    /// `raw_sse_tx` is the broadcast channel for re-broadcasting raw upstream
    /// opencode SSE events to web clients.
    pub fn new(
        config: &Config,
        event_tx: broadcast::Sender<WebEvent>,
        raw_sse_tx: broadcast::Sender<String>,
    ) -> Self {
        // Open SQLite database and run one-time migration from legacy JSON.
        let db = Db::open().unwrap_or_else(|e| {
            panic!("failed to open assistant database: {e}");
        });
        super::db::migrate::run_migration(&db);

        // Load persisted state from SQLite.
        let missions: HashMap<String, Mission> = db
            .list_missions()
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect();
        let personal_memory: HashMap<String, PersonalMemoryItem> = db
            .list_memory()
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect();
        let autonomy_settings = db.load_autonomy_settings();
        let routines: HashMap<String, RoutineDefinition> = db
            .list_routines()
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect();
        let routine_runs = db.list_routine_runs();
        let delegated_work: HashMap<String, DelegatedWorkItem> = db
            .list_delegated_work()
            .into_iter()
            .map(|d| (d.id.clone(), d))
            .collect();
        let workspaces: HashMap<String, WorkspaceSnapshot> = db
            .list_workspaces()
            .into_iter()
            .map(|ws| (ws.name.clone(), ws))
            .collect();
        let signals = db.list_signals(100);

        info!(
            "loaded from SQLite: {} missions, {} memory, {} routines, {} delegated, {} workspaces, {} signals",
            missions.len(),
            personal_memory.len(),
            routines.len(),
            delegated_work.len(),
            workspaces.len(),
            signals.len(),
        );

        let projects: Vec<WebProject> = config
            .projects
            .iter()
            .map(|entry| WebProject {
                name: entry.name.clone(),
                path: PathBuf::from(&entry.path),
                sessions: Vec::new(),
                active_session: None,
                git_branch: String::new(),
            })
            .collect();

        let inner = Arc::new(RwLock::new(WebStateInner {
            active_project: 0,
            projects,
            panels: WebPanelVisibility {
                sidebar: true,
                terminal_pane: true,
                neovim_pane: true,
                integrated_terminal: true,
                git_panel: true,
            },
            focused: "TerminalPane".to_string(),
            session_stats: HashMap::new(),
            busy_sessions: HashSet::new(),
            theme: None,
            session_watchers: HashMap::new(),
            watcher_pending: HashMap::new(),
            watcher_idle_since: HashMap::new(),
            session_children: HashMap::new(),
            file_snapshots: HashMap::new(),
            file_edits: HashMap::new(),
            connected_clients: HashMap::new(),
            activity_log: HashMap::new(),
            missions,
            personal_memory,
            autonomy_settings,
            routines,
            routine_runs,
            delegated_work,
            workspaces,
            signals,
            pending_permissions: HashMap::new(),
            pending_questions: HashMap::new(),
        }));

        let (persist_tx, persist_rx) = mpsc::unbounded_channel();

        let handle = Self { inner, event_tx, raw_sse_tx, db, persist_tx };

        // Spawn background tasks
        handle.spawn_persist_worker(persist_rx);
        handle.spawn_session_poller();
        handle.spawn_opencode_sse_listener();

        handle
    }
}

pub(super) fn uuid_like_id() -> String {
    format!(
        "{:x}{:x}",
        rand::random::<u64>(),
        rand::random::<u64>()
    )
}
