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

mod assistant_autonomy;
mod assistant_memory;
mod assistant_routine_exec;
mod assistant_routines;
mod assistant_send;
mod background;
mod db_sync;
mod file_edits;
mod kanban;
mod kanban_pipeline;
mod kanban_pipeline_brief;
mod kanban_query;
pub(crate) use kanban::KanbanError;
mod active_memory;
mod mutations;
mod presence;
mod queries;
pub(super) mod scheduler;
mod sse;
mod sse_handler;
mod status;
mod watchers;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::sync::{broadcast, RwLock};
use tokio::task::AbortHandle;


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
    /// False until every configured project's initial session list is hydrated.
    /// This prevents an empty first snapshot from being mistaken for a new session.
    pub(super) startup_ready: bool,
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
    /// When a turn was dispatched to a runner, for sessions whose runner has
    /// not reported the turn yet. Shields that gap from the status sweep.
    pub(super) turn_dispatch: HashMap<String, Instant>,
    /// Current theme colors (dark + light variants) for the web frontend.
    pub(super) theme: Option<WebThemePair>,
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
    // ── Session Continuity: presence ─────────────────────────────
    /// Connected clients, keyed by client_id.
    pub(super) connected_clients: HashMap<String, ClientPresence>,
    /// Saved personal memory items, keyed by ID.
    pub(super) personal_memory: HashMap<String, PersonalMemoryItem>,
    /// Current autonomy settings.
    pub(super) autonomy_settings: AutonomySettings,
    /// Saved routines.
    pub(super) routines: HashMap<String, RoutineDefinition>,
    /// Routine execution history.
    pub(super) routine_runs: Vec<RoutineRunRecord>,
    // ── Pending permissions & questions (ephemeral, from SSE) ──
    /// Pending permission requests, keyed by request ID.
    /// Stored as raw JSON so the frontend receives the same shape as SSE events.
    pub(super) pending_permissions: HashMap<String, serde_json::Value>,
    /// Pending question requests, keyed by request ID.
    pub(super) pending_questions: HashMap<String, serde_json::Value>,
    // ── Session indicator state (parity with upstream opencode) ──
    /// Sessions that have encountered an error (session_id → error message).
    pub(super) error_sessions: HashMap<String, String>,
    /// Sessions that need user input (pending permission or question).
    /// Derived from `pending_permissions` + `pending_questions` session IDs.
    pub(super) input_sessions: HashSet<String>,
    // ── Unseen / unread tracking (parity with upstream opencode) ──
    /// Sessions with unseen activity (session_id → unseen event count).
    /// Incremented on `session.idle` / `session.error` if the session is not
    /// the currently active session for its project.  Cleared when a client
    /// selects/views the session.
    pub(super) unseen_sessions: HashMap<String, usize>,
    // ── Idle-routine cooldown ───────────────────────────────────
    /// Last time each OnSessionIdle routine fired, to prevent self-loops.
    pub(super) routine_idle_cooldown: HashMap<String, Instant>,
    /// Runtime runner label for each logical session.
    pub(super) session_runners: HashMap<String, String>,
    /// Runner used when a session has not made an explicit selection yet.
    pub(super) default_runner: String,
    /// Sessions that have already been given their session instructions.
    ///
    /// Instructions open a session; re-sending them on every turn is what this
    /// replaced. In-memory is enough: on a cold start the send path falls back
    /// to asking the engine whether the session already has turns, which is the
    /// authoritative answer anyway.
    pub(super) instructions_sent: HashSet<String>,
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
    /// Broadcast channel for editor-specific file-change events.
    pub(super) editor_tx: Option<broadcast::Sender<EditorEvent>>,
    /// SQLite database handle (replaces JSON persistence).
    pub(super) db: Db,
    /// Channel to trigger async DB writes (debounced).
    pub(super) persist_tx: mpsc::UnboundedSender<()>,
    /// Native runner registry used to discover non-OpenCode sessions during
    /// the same background refresh that hydrates the sidebar.
    pub(super) runner_registry: Option<Arc<crate::runner::RunnerRegistry>>,
}

impl WebStateHandle {
    /// Process an event emitted by a non-default runner. This keeps Codex
    /// approvals, busy state, stats, and file activity on the same web-state
    /// path as the default OpenCode SSE stream.
    pub async fn handle_runner_event(&self, data: &str, project_dir: &str) {
        sse_handler::handle_web_sse_event(self, data, project_dir).await;
    }

    /// Every configured project directory, as the runners address them.
    pub async fn project_directories(&self) -> Vec<String> {
        let inner = self.inner.read().await;
        inner
            .projects
            .iter()
            .map(|project| project.path.to_string_lossy().to_string())
            .collect()
    }

    pub async fn directory_for_session(&self, session_id: &str) -> Option<String> {
        let inner = self.inner.read().await;
        inner.projects.iter().find_map(|project| {
            project
                .sessions
                .iter()
                .any(|session| session.id == session_id)
                .then(|| project.path.to_string_lossy().to_string())
        })
    }
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
        runner_registry: Arc<crate::runner::RunnerRegistry>,
    ) -> Self {
        // Open SQLite database and run one-time migration from legacy JSON.
        let db = Db::open().unwrap_or_else(|e| {
            panic!("failed to open assistant database: {e}");
        });
        super::db::migrate_legacy_json::run_migration(&db);

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

        let inner = Arc::new(RwLock::new(Self::build_inner(&db, projects)));

        let (persist_tx, persist_rx) = mpsc::unbounded_channel();

        let handle = Self {
            inner,
            event_tx,
            raw_sse_tx,
            editor_tx: None,
            db,
            persist_tx,
            runner_registry: Some(runner_registry),
        };

        // Spawn background tasks
        handle.spawn_persist_worker(persist_rx);
        handle.spawn_session_poller();
        handle.spawn_status_poller();
        handle.spawn_opencode_sse_listener();
        handle.spawn_routine_scheduler();
        handle.spawn_presence_cleanup();

        handle
    }

    /// Build the inner mutable state, loading persisted collections from `db`.
    /// Shared by the production constructor and the test constructors.
    fn build_inner(db: &Db, projects: Vec<WebProject>) -> WebStateInner {
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

        WebStateInner {
            startup_ready: false,
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
            turn_dispatch: HashMap::new(),
            theme: None,
            session_watchers: HashMap::new(),
            watcher_pending: HashMap::new(),
            watcher_idle_since: HashMap::new(),
            session_children: HashMap::new(),
            file_snapshots: HashMap::new(),
            file_edits: HashMap::new(),
            connected_clients: HashMap::new(),
            personal_memory,
            autonomy_settings,
            routines,
            routine_runs,
            pending_permissions: HashMap::new(),
            pending_questions: HashMap::new(),
            error_sessions: HashMap::new(),
            input_sessions: HashSet::new(),
            unseen_sessions: HashMap::new(),
            routine_idle_cooldown: HashMap::new(),
            session_runners: HashMap::new(),
            instructions_sent: HashSet::new(),
            default_runner: "opencode".to_string(),
        }
    }

    /// Test-only constructor: in-memory DB, no background pollers/scheduler.
    ///
    /// The persist channel receiver is dropped, so `mark_dirty()` calls are
    /// silently discarded (no persistence worker runs).
    #[cfg(test)]
    pub(crate) fn new_test() -> Self {
        Self::new_test_with_projects(Vec::new())
    }

    /// Test-only constructor with pre-populated projects (`(name, path)`).
    #[cfg(test)]
    pub(crate) fn new_test_with_projects(projects: Vec<(String, PathBuf)>) -> Self {
        let db = Db::open_memory().expect("open in-memory test db");
        let (event_tx, _) = broadcast::channel::<WebEvent>(1000);
        let (raw_sse_tx, _) = broadcast::channel::<String>(2000);
        let projects: Vec<WebProject> = projects
            .into_iter()
            .map(|(name, path)| WebProject {
                name,
                path,
                sessions: Vec::new(),
                active_session: None,
                git_branch: String::new(),
            })
            .collect();
        let inner = Arc::new(RwLock::new(Self::build_inner(&db, projects)));
        let (persist_tx, _persist_rx) = mpsc::unbounded_channel();
        Self {
            inner,
            event_tx,
            raw_sse_tx,
            editor_tx: None,
            db,
            persist_tx,
            runner_registry: None,
        }
    }

    /// Access the underlying DB handle (tests only).
    #[cfg(test)]
    pub(crate) fn db_for_test(&self) -> &Db {
        &self.db
    }

    /// Subscribe to internal web-state events (e.g. `RoutineUpdated`).
    ///
    /// Used by the TUI to receive push notifications when the web state
    /// changes, without going through HTTP/SSE.
    pub fn subscribe_events(&self) -> broadcast::Receiver<WebEvent> {
        self.event_tx.subscribe()
    }

    /// Attach the editor broadcast channel after `ServerState` construction.
    pub fn set_editor_tx(&mut self, tx: broadcast::Sender<EditorEvent>) {
        self.editor_tx = Some(tx);
    }

    /// Emit a file-changed event on the editor SSE channel (if attached).
    pub(super) fn emit_editor_file_changed(&self, path: &str, source: &str) {
        if let Some(ref tx) = self.editor_tx {
            let _ = tx.send(EditorEvent::FileChanged {
                path: path.to_string(),
                source: source.to_string(),
            });
        }
    }
}

pub(super) fn uuid_like_id() -> String {
    format!("{:x}{:x}", rand::random::<u64>(), rand::random::<u64>())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;

#[cfg(test)]
#[path = "mod_build_inner_tests.rs"]
mod mod_build_inner_tests;
