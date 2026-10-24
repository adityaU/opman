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
mod background_disk;
mod background_hydration;
mod background_sessions;
mod background_started;
mod db_sync;
mod file_edits;
mod initialization;
mod kanban;
mod kanban_pipeline;
mod kanban_pipeline_brief;
mod kanban_query;
pub(crate) use kanban::KanbanError;
mod active_memory;
mod instruction_delivery;
mod mutations;
mod panels;
mod presence;
mod queries;
pub(super) mod scheduler;
mod session_page;
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
    /// Tracks whether the initial session hydration pass has settled.
    /// This prevents an empty first snapshot from being mistaken for a new session.
    pub(super) startup_hydration: background_hydration::StartupHydration,
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

    /// Publish an event opman raised itself, on the path an upstream one would have taken.
    ///
    /// Both halves matter and neither is optional: the broadcast is what an open browser
    /// sees now, and the state pass is what `GET /api/pending` replays after a reload. A
    /// question raised through the `ask` MCP server has no upstream stream to arrive on,
    /// so it comes through here instead — and is then indistinguishable from an
    /// engine-emitted one, which is why the frontend needs no special case.
    pub async fn publish_event(&self, event: &serde_json::Value, project_dir: &str) {
        let Ok(data) = serde_json::to_string(event) else {
            return;
        };
        let _ = self.raw_sse_tx.send(data.clone());
        sse_handler::handle_web_sse_event(self, &data, project_dir).await;
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

pub(super) fn uuid_like_id() -> String {
    format!("{:x}{:x}", rand::random::<u64>(), rand::random::<u64>())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;

#[cfg(test)]
#[path = "mod_build_inner_tests.rs"]
mod mod_build_inner_tests;

#[cfg(test)]
#[path = "background_started_tests.rs"]
mod background_started_tests;

#[cfg(test)]
#[path = "background_disk_tests.rs"]
mod background_disk_tests;
