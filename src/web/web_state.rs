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

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use futures::future::join_all;
use tokio::sync::{broadcast, RwLock};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use tracing::{debug, info, warn};

use crate::api::ApiClient;
use crate::app::{base_url, SessionInfo};
use crate::config::{Config, ProjectEntry};

use super::types::*;

// ── Internal state ──────────────────────────────────────────────────

/// Per-project data maintained by the web state manager.
#[derive(Debug, Clone)]
struct WebProject {
    name: String,
    path: PathBuf,
    sessions: Vec<SessionInfo>,
    active_session: Option<String>,
    git_branch: String,
}

/// Inner mutable state protected by `RwLock`.
struct WebStateInner {
    projects: Vec<WebProject>,
    active_project: usize,
    /// Panel visibility (sidebar, terminal_pane, neovim_pane, integrated_terminal, git_panel).
    panels: WebPanelVisibility,
    /// Currently focused panel name.
    focused: String,
    /// Per-session cost/token stats, keyed by session ID.
    session_stats: HashMap<String, WebSessionStats>,
    /// Set of session IDs currently busy.
    busy_sessions: HashSet<String>,
    /// Current theme colors (hex strings) for the web frontend.
    theme: Option<WebThemeColors>,
    // ── Watcher state ────────────────────────────────────────────
    /// Active watcher configurations, keyed by session ID.
    session_watchers: HashMap<String, WatcherConfigInternal>,
    /// Pending watcher timers (abort handles for delayed continuation sends).
    watcher_pending: HashMap<String, AbortHandle>,
    /// When each watched session went idle (for countdown display).
    watcher_idle_since: HashMap<String, Instant>,
    /// Parent→children mapping for subagent suppression.
    session_children: HashMap<String, HashSet<String>>,
    // ── File edit tracking (diff review) ─────────────────────────
    /// Per-session file snapshots: session_id → (file_path → original_content).
    /// Stores the content of a file *before* the first edit in the session.
    file_snapshots: HashMap<String, HashMap<String, String>>,
    /// Per-session ordered list of file edit events.
    file_edits: HashMap<String, Vec<FileEditRecord>>,
    // ── Session Continuity: presence + activity ──────────────────
    /// Connected clients, keyed by client_id.
    connected_clients: HashMap<String, ClientPresence>,
    /// Per-session recent activity events (ring buffer, max 200 per session).
    activity_log: HashMap<String, Vec<ActivityEventPayload>>,
    /// Saved missions, keyed by ID.
    missions: HashMap<String, Mission>,
    /// Saved personal memory items, keyed by ID.
    personal_memory: HashMap<String, PersonalMemoryItem>,
    /// Current autonomy settings.
    autonomy_settings: AutonomySettings,
    /// Saved routines.
    routines: HashMap<String, RoutineDefinition>,
    /// Routine execution history.
    routine_runs: Vec<RoutineRunRecord>,
    /// Delegated work items.
    delegated_work: HashMap<String, DelegatedWorkItem>,
    // ── Workspace Snapshots ─────────────────────────────────────
    /// Saved workspace snapshots, keyed by name.
    workspaces: HashMap<String, WorkspaceSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct PersistedAssistantState {
    #[serde(default)]
    missions: HashMap<String, Mission>,
    #[serde(default)]
    personal_memory: HashMap<String, PersonalMemoryItem>,
    #[serde(default)]
    autonomy_settings: Option<AutonomySettings>,
    #[serde(default)]
    routines: HashMap<String, RoutineDefinition>,
    #[serde(default)]
    routine_runs: Vec<RoutineRunRecord>,
    #[serde(default)]
    delegated_work: HashMap<String, DelegatedWorkItem>,
    #[serde(default)]
    workspaces: HashMap<String, WorkspaceSnapshot>,
}

/// Internal watcher config (stored on the server side).
#[derive(Clone, Debug)]
struct WatcherConfigInternal {
    session_id: String,
    project_idx: usize,
    idle_timeout_secs: u64,
    continuation_message: String,
    include_original: bool,
    original_message: Option<String>,
    hang_message: String,
    hang_timeout_secs: u64,
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
    inner: Arc<RwLock<WebStateInner>>,
    /// Broadcast channel for notifying SSE clients of state changes.
    event_tx: broadcast::Sender<WebEvent>,
    persist_path: PathBuf,
    persist_tx: mpsc::UnboundedSender<()>,
}

impl WebStateHandle {
    /// Create the web state from config, start background pollers.
    ///
    /// `event_tx` is the broadcast channel that SSE clients subscribe to.
    pub fn new(config: &Config, event_tx: broadcast::Sender<WebEvent>) -> Self {
        let persist_path = assistant_state_path();
        let persisted = load_persisted_assistant_state(&persist_path);

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
            missions: persisted.missions,
            personal_memory: persisted.personal_memory,
            autonomy_settings: persisted.autonomy_settings.unwrap_or(AutonomySettings {
                mode: AutonomyMode::Observe,
                updated_at: Utc::now().to_rfc3339(),
            }),
            routines: persisted.routines,
            routine_runs: persisted.routine_runs,
            delegated_work: persisted.delegated_work,
            workspaces: persisted.workspaces,
        }));

        let (persist_tx, persist_rx) = mpsc::unbounded_channel();

        let handle = Self { inner, event_tx, persist_path, persist_tx };

        // Spawn background tasks
        handle.spawn_persist_worker(persist_rx);
        handle.spawn_session_poller();
        handle.spawn_opencode_sse_listener();

        handle
    }

    // ── Queries ─────────────────────────────────────────────────────

    /// Build a complete `WebAppState` snapshot for the `/api/state` endpoint.
    pub async fn get_state(&self) -> WebAppState {
        let inner = self.inner.read().await;
        let projects = inner
            .projects
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let busy: Vec<String> = p
                    .sessions
                    .iter()
                    .filter(|s| inner.busy_sessions.contains(&s.id))
                    .map(|s| s.id.clone())
                    .collect();
                WebProjectInfo {
                    name: p.name.clone(),
                    path: p.path.to_string_lossy().to_string(),
                    index: i,
                    active_session: p.active_session.clone(),
                    sessions: p
                        .sessions
                        .iter()
                        .map(|s| WebSessionInfo {
                            id: s.id.clone(),
                            title: s.title.clone(),
                            parent_id: s.parent_id.clone(),
                            directory: s.directory.clone(),
                            time: WebSessionTime {
                                created: s.time.created,
                                updated: s.time.updated,
                            },
                        })
                        .collect(),
                    git_branch: p.git_branch.clone(),
                    busy_sessions: busy,
                }
            })
            .collect();

        WebAppState {
            projects,
            active_project: inner.active_project,
            panels: inner.panels.clone(),
            focused: inner.focused.clone(),
        }
    }

    /// Get session stats for a given session ID.
    pub async fn get_session_stats(&self, session_id: &str) -> Option<WebSessionStats> {
        let inner = self.inner.read().await;
        inner.session_stats.get(session_id).cloned()
    }

    /// Build a flat overview of all sessions across all projects.
    pub async fn get_sessions_overview(&self) -> SessionsOverviewResponse {
        let inner = self.inner.read().await;
        let mut sessions = Vec::new();
        let mut busy_count = 0usize;

        for (i, project) in inner.projects.iter().enumerate() {
            for s in &project.sessions {
                let is_busy = inner.busy_sessions.contains(&s.id);
                if is_busy {
                    busy_count += 1;
                }
                sessions.push(SessionOverviewEntry {
                    id: s.id.clone(),
                    title: s.title.clone(),
                    parent_id: s.parent_id.clone(),
                    project_name: project.name.clone(),
                    project_index: i,
                    directory: s.directory.clone(),
                    is_busy,
                    time: WebSessionTime {
                        created: s.time.created,
                        updated: s.time.updated,
                    },
                    stats: inner.session_stats.get(&s.id).cloned(),
                });
            }
        }

        // Sort by most recently updated first
        sessions.sort_by(|a, b| b.time.updated.cmp(&a.time.updated));
        let total = sessions.len();
        SessionsOverviewResponse {
            sessions,
            total,
            busy_count,
        }
    }

    /// Build a tree of sessions showing parent/child relationships.
    pub async fn get_sessions_tree(&self) -> SessionsTreeResponse {
        let inner = self.inner.read().await;

        // Collect all sessions with their metadata
        struct FlatSession {
            id: String,
            title: String,
            parent_id: String,
            project_name: String,
            project_index: usize,
            is_busy: bool,
            stats: Option<WebSessionStats>,
        }

        let mut all: Vec<FlatSession> = Vec::new();
        for (i, project) in inner.projects.iter().enumerate() {
            for s in &project.sessions {
                all.push(FlatSession {
                    id: s.id.clone(),
                    title: s.title.clone(),
                    parent_id: s.parent_id.clone(),
                    project_name: project.name.clone(),
                    project_index: i,
                    is_busy: inner.busy_sessions.contains(&s.id),
                    stats: inner.session_stats.get(&s.id).cloned(),
                });
            }
        }

        let total = all.len();
        let id_set: std::collections::HashSet<&str> =
            all.iter().map(|s| s.id.as_str()).collect();

        // Build children lookup: parent_id -> [child indices]
        let mut children_map: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        let mut root_indices = Vec::new();

        for (idx, s) in all.iter().enumerate() {
            if s.parent_id.is_empty() || !id_set.contains(s.parent_id.as_str()) {
                root_indices.push(idx);
            } else {
                children_map
                    .entry(s.parent_id.clone())
                    .or_default()
                    .push(idx);
            }
        }

        // Recursive tree builder
        fn build_node(
            idx: usize,
            all: &[FlatSession],
            children_map: &std::collections::HashMap<String, Vec<usize>>,
        ) -> SessionTreeNode {
            let s = &all[idx];
            let children = children_map
                .get(&s.id)
                .map(|child_idxs| {
                    child_idxs
                        .iter()
                        .map(|&ci| build_node(ci, all, children_map))
                        .collect()
                })
                .unwrap_or_default();
            SessionTreeNode {
                id: s.id.clone(),
                title: s.title.clone(),
                project_name: s.project_name.clone(),
                project_index: s.project_index,
                is_busy: s.is_busy,
                stats: s.stats.clone(),
                children,
            }
        }

        let roots: Vec<SessionTreeNode> = root_indices
            .iter()
            .map(|&ri| build_node(ri, &all, &children_map))
            .collect();

        SessionsTreeResponse { roots, total }
    }

    /// Get all tracked file edits for a session.
    pub async fn get_file_edits(&self, session_id: &str) -> Vec<FileEditRecord> {
        let inner = self.inner.read().await;
        inner
            .file_edits
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Record a file edit event for a session.
    /// Reads the file from disk, stores original snapshot on first edit,
    /// and records the edit with before/after content.
    pub async fn record_file_edit(
        &self,
        session_id: &str,
        file_path: &str,
        project_dir: Option<&std::path::Path>,
    ) {
        // Resolve absolute path
        let abs_path = if std::path::Path::new(file_path).is_absolute() {
            std::path::PathBuf::from(file_path)
        } else if let Some(dir) = project_dir {
            dir.join(file_path)
        } else {
            std::path::PathBuf::from(file_path)
        };

        // Read current file content
        let new_content = match tokio::fs::read_to_string(&abs_path).await {
            Ok(c) => c,
            Err(_) => return, // File doesn't exist or can't be read
        };

        // Check if we already have a snapshot (read lock first to avoid holding
        // a write lock across the git subprocess .await).
        let needs_original = {
            let inner = self.inner.read().await;
            inner
                .file_snapshots
                .get(session_id)
                .and_then(|snaps| snaps.get(file_path))
                .cloned()
        };

        let original_content = match needs_original {
            Some(existing) => existing,
            None => {
                // First edit: fetch git original content BEFORE acquiring write lock
                let original = Self::get_git_original(&abs_path, project_dir).await
                    .unwrap_or_else(|| new_content.clone());
                // Store snapshot under write lock (brief, no .await inside)
                let mut inner = self.inner.write().await;
                let snapshots = inner
                    .file_snapshots
                    .entry(session_id.to_string())
                    .or_default();
                // Double-check: another task may have inserted while we awaited
                if let Some(existing) = snapshots.get(file_path) {
                    existing.clone()
                } else {
                    snapshots.insert(file_path.to_string(), original.clone());
                    drop(inner); // release write lock early
                    original
                }
            }
        };

        // Record the edit (brief write lock, no .await inside)
        {
            let mut inner = self.inner.write().await;
            let edits = inner
                .file_edits
                .entry(session_id.to_string())
                .or_default();
            let index = edits.len();
            let timestamp = chrono::Utc::now().to_rfc3339();

            edits.push(FileEditRecord {
                path: file_path.to_string(),
                original_content,
                new_content,
                timestamp,
                index,
            });
        }
    }

    /// Try to get the original file content from git (HEAD version).
    async fn get_git_original(
        abs_path: &std::path::Path,
        project_dir: Option<&std::path::Path>,
    ) -> Option<String> {
        let dir = project_dir?;
        // Make path relative to project dir
        let rel_path = abs_path.strip_prefix(dir).ok()?;
        let output = tokio::process::Command::new("git")
            .arg("show")
            .arg(format!("HEAD:{}", rel_path.display()))
            .current_dir(dir)
            .output()
            .await
            .ok()?;
        if output.status.success() {
            String::from_utf8(output.stdout).ok()
        } else {
            None
        }
    }

    /// Get the working directory of the active project.
    pub async fn get_working_dir(&self) -> Option<PathBuf> {
        let inner = self.inner.read().await;
        inner
            .projects
            .get(inner.active_project)
            .map(|p| p.path.clone())
    }

    pub async fn active_project_index(&self) -> usize {
        let inner = self.inner.read().await;
        inner.active_project
    }

    /// Get the working directory of a specific project by index.
    pub async fn get_project_working_dir(&self, project_idx: usize) -> Option<PathBuf> {
        let inner = self.inner.read().await;
        inner.projects.get(project_idx).map(|p| p.path.clone())
    }

    /// Get all sessions for a specific project, with project metadata.
    /// Returns (project_path, project_name, Vec<(session_id, session_title)>).
    pub async fn get_project_sessions(
        &self,
        project_idx: usize,
    ) -> Option<(PathBuf, String, Vec<(String, String)>)> {
        let inner = self.inner.read().await;
        let project = inner.projects.get(project_idx)?;
        let sessions: Vec<(String, String)> = project
            .sessions
            .iter()
            .map(|s| (s.id.clone(), s.title.clone()))
            .collect();
        Some((project.path.clone(), project.name.clone(), sessions))
    }

    /// Get the current theme colors (if set).
    pub async fn get_theme(&self) -> Option<WebThemeColors> {
        let inner = self.inner.read().await;
        inner.theme.clone()
    }

    /// Set the theme colors and broadcast a `ThemeChanged` event to SSE clients.
    pub async fn set_theme(&self, theme: WebThemeColors) {
        {
            let mut inner = self.inner.write().await;
            inner.theme = Some(theme.clone());
        }
        let _ = self.event_tx.send(WebEvent::ThemeChanged(theme));
    }

    // ── Watcher management ──────────────────────────────────────

    /// Create or update a watcher for a session.
    pub async fn create_watcher(&self, req: WatcherConfigRequest) -> WatcherConfigResponse {
        let config = WatcherConfigInternal {
            session_id: req.session_id.clone(),
            project_idx: req.project_idx,
            idle_timeout_secs: req.idle_timeout_secs,
            continuation_message: req.continuation_message.clone(),
            include_original: req.include_original,
            original_message: req.original_message.clone(),
            hang_message: req.hang_message.clone(),
            hang_timeout_secs: req.hang_timeout_secs,
        };

        let mut inner = self.inner.write().await;
        inner.session_watchers.insert(req.session_id.clone(), config);

        // Determine current status
        let is_busy = inner.busy_sessions.contains(&req.session_id);
        let status = if is_busy { "running" } else { "waiting" }.to_string();

        drop(inner);

        let _ = self.event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
            session_id: req.session_id.clone(),
            action: "created".to_string(),
            idle_since_secs: None,
        }));

        info!(session_id = %req.session_id, "Watcher created");

        WatcherConfigResponse {
            session_id: req.session_id,
            project_idx: req.project_idx,
            idle_timeout_secs: req.idle_timeout_secs,
            continuation_message: req.continuation_message,
            include_original: req.include_original,
            original_message: req.original_message,
            hang_message: req.hang_message,
            hang_timeout_secs: req.hang_timeout_secs,
            status,
            idle_since_secs: None,
        }
    }

    /// Delete a watcher for a session.
    pub async fn delete_watcher(&self, session_id: &str) -> bool {
        let mut inner = self.inner.write().await;
        let removed = inner.session_watchers.remove(session_id).is_some();
        if removed {
            // Cancel any pending timer
            if let Some(handle) = inner.watcher_pending.remove(session_id) {
                handle.abort();
            }
            inner.watcher_idle_since.remove(session_id);

            drop(inner);

            let _ = self.event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
                session_id: session_id.to_string(),
                action: "deleted".to_string(),
                idle_since_secs: None,
            }));

            info!(session_id = %session_id, "Watcher deleted");
        }
        removed
    }

    /// List all active watchers with their current status.
    pub async fn list_watchers(&self) -> Vec<WatcherListEntry> {
        let inner = self.inner.read().await;
        let mut entries = Vec::new();

        for (sid, config) in &inner.session_watchers {
            let is_busy = inner.busy_sessions.contains(sid);
            let idle_since = inner.watcher_idle_since.get(sid);

            let (status, idle_since_secs) = if is_busy {
                ("running".to_string(), None)
            } else if let Some(since) = idle_since {
                let elapsed = since.elapsed().as_secs();
                ("idle_countdown".to_string(), Some(elapsed))
            } else {
                ("waiting".to_string(), None)
            };

            // Find session title and project name
            let mut title = sid.clone();
            let mut project_name = String::new();
            if let Some(project) = inner.projects.get(config.project_idx) {
                project_name = project.name.clone();
                if let Some(session) = project.sessions.iter().find(|s| s.id == *sid) {
                    title = session.title.clone();
                }
            }

            entries.push(WatcherListEntry {
                session_id: sid.clone(),
                session_title: title,
                project_name,
                idle_timeout_secs: config.idle_timeout_secs,
                status,
                idle_since_secs,
            });
        }

        entries
    }

    /// Get watcher config for a specific session.
    pub async fn get_watcher(&self, session_id: &str) -> Option<WatcherConfigResponse> {
        let inner = self.inner.read().await;
        let config = inner.session_watchers.get(session_id)?;
        let is_busy = inner.busy_sessions.contains(session_id);
        let idle_since = inner.watcher_idle_since.get(session_id);

        let (status, idle_since_secs) = if is_busy {
            ("running".to_string(), None)
        } else if let Some(since) = idle_since {
            let elapsed = since.elapsed().as_secs();
            ("idle_countdown".to_string(), Some(elapsed))
        } else {
            ("waiting".to_string(), None)
        };

        Some(WatcherConfigResponse {
            session_id: config.session_id.clone(),
            project_idx: config.project_idx,
            idle_timeout_secs: config.idle_timeout_secs,
            continuation_message: config.continuation_message.clone(),
            include_original: config.include_original,
            original_message: config.original_message.clone(),
            hang_message: config.hang_message.clone(),
            hang_timeout_secs: config.hang_timeout_secs,
            status,
            idle_since_secs,
        })
    }

    /// Get all sessions formatted for the watcher session picker.
    pub async fn get_watcher_sessions(&self) -> Vec<WatcherSessionEntry> {
        let inner = self.inner.read().await;
        let mut entries = Vec::new();

        for (idx, project) in inner.projects.iter().enumerate() {
            for session in &project.sessions {
                let is_current = project.active_session.as_deref() == Some(&session.id)
                    && idx == inner.active_project;
                let is_active = inner.busy_sessions.contains(&session.id);
                let has_watcher = inner.session_watchers.contains_key(&session.id);

                entries.push(WatcherSessionEntry {
                    session_id: session.id.clone(),
                    title: session.title.clone(),
                    project_name: project.name.clone(),
                    project_idx: idx,
                    is_current,
                    is_active,
                    has_watcher,
                });
            }
        }

        entries
    }

    /// Try to trigger a watcher when a session goes idle.
    /// Called from the SSE handler when `session.status` → `idle`.
    async fn try_trigger_watcher(&self, session_id: &str) {
        let inner = self.inner.read().await;
        let watcher = match inner.session_watchers.get(session_id) {
            Some(w) => w.clone(),
            None => return,
        };

        // Check for active children (suppress if subagent sessions still running)
        let has_active_children = inner
            .session_children
            .get(session_id)
            .map(|children| children.iter().any(|c| inner.busy_sessions.contains(c)))
            .unwrap_or(false);

        drop(inner);

        if has_active_children {
            info!(
                session_id = %session_id,
                "Watcher: suppressed — subagent sessions still active"
            );
            let mut inner = self.inner.write().await;
            inner.watcher_idle_since.remove(session_id);
            return;
        }

        // Cancel any existing pending timer
        {
            let mut inner = self.inner.write().await;
            if let Some(prev_handle) = inner.watcher_pending.remove(session_id) {
                prev_handle.abort();
            }
            // Record when idle countdown started
            inner.watcher_idle_since.insert(session_id.to_string(), Instant::now());
        }

        let _ = self.event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
            session_id: session_id.to_string(),
            action: "countdown".to_string(),
            idle_since_secs: Some(0),
        }));

        let timeout = watcher.idle_timeout_secs;
        let msg = watcher.continuation_message.clone();
        let original = if watcher.include_original {
            watcher.original_message.clone()
        } else {
            None
        };
        let api = crate::api::ApiClient::new();
        let base_url = crate::app::base_url().to_string();

        let project_dir = {
            let inner = self.inner.read().await;
            inner
                .projects
                .get(watcher.project_idx)
                .map(|p| p.path.display().to_string())
                .unwrap_or_default()
        };

        let sid = session_id.to_string();
        let event_tx = self.event_tx.clone();

        info!(
            session_id = %sid,
            timeout_secs = timeout,
            "Watcher: scheduling continuation message after idle"
        );

        let handle = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(timeout)).await;
            info!(session_id = %sid, "Watcher: sending continuation message");
            let mut full_msg = String::new();
            if let Some(orig) = original {
                full_msg.push_str(&format!("[Original message]: {}\n\n", orig));
            }
            full_msg.push_str(&msg);
            if let Err(e) = api
                .send_system_message_async(&base_url, &project_dir, &sid, &full_msg)
                .await
            {
                warn!("Watcher failed to send message to {}: {}", sid, e);
            }
            let _ = event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
                session_id: sid.clone(),
                action: "triggered".to_string(),
                idle_since_secs: None,
            }));
        });

        let mut inner = self.inner.write().await;
        inner.watcher_pending.insert(session_id.to_string(), handle.abort_handle());
    }

    /// Cancel a pending watcher timer (called when session goes busy).
    async fn cancel_watcher_timer(&self, session_id: &str) {
        let mut inner = self.inner.write().await;
        if inner.session_watchers.contains_key(session_id) {
            if let Some(handle) = inner.watcher_pending.remove(session_id) {
                handle.abort();
            }
            inner.watcher_idle_since.remove(session_id);

            drop(inner);

            let _ = self.event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
                session_id: session_id.to_string(),
                action: "cancelled".to_string(),
                idle_since_secs: None,
            }));
        }
    }

    // ── Mutations ───────────────────────────────────────────────────

    /// Switch the active project.
    pub async fn switch_project(&self, index: usize) -> bool {
        let mut inner = self.inner.write().await;
        if index < inner.projects.len() {
            inner.active_project = index;
            let _ = self.event_tx.send(WebEvent::StateChanged);
            true
        } else {
            false
        }
    }

    /// Add a new project. Returns `Ok((index, name))` on success.
    ///
    /// Validates the path is a directory, checks for duplicates, adds to the
    /// in-memory state and persists to the config file.
    pub async fn add_project(&self, path_str: &str, name: Option<&str>) -> Result<(usize, String), String> {
        let path = std::path::PathBuf::from(path_str);

        // Canonicalize the path
        let canonical = std::fs::canonicalize(&path)
            .map_err(|e| format!("Invalid path: {e}"))?;

        if !canonical.is_dir() {
            return Err("Path is not a directory".into());
        }

        // Check for duplicates
        {
            let inner = self.inner.read().await;
            for project in &inner.projects {
                if project.path == canonical {
                    return Err("Project already exists".into());
                }
            }
        }

        // Derive name from directory if not provided
        let project_name = match name {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => canonical
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path_str.to_string()),
        };

        // Add to in-memory state
        let index = {
            let mut inner = self.inner.write().await;
            let project = WebProject {
                name: project_name.clone(),
                path: canonical.clone(),
                sessions: Vec::new(),
                active_session: None,
                git_branch: String::new(),
            };
            inner.projects.push(project);
            inner.projects.len() - 1
        };

        // Persist to config file
        if let Ok(mut config) = Config::load() {
            config.projects.push(ProjectEntry {
                name: project_name.clone(),
                path: canonical.to_string_lossy().to_string(),
                terminal_command: None,
            });
            if let Err(e) = config.save() {
                warn!("Failed to save config after adding project: {e}");
            }
        }

        let _ = self.event_tx.send(WebEvent::StateChanged);
        Ok((index, project_name))
    }

    /// Remove a project by index. Returns `Ok(())` on success.
    ///
    /// Removes from in-memory state and persists to the config file.
    pub async fn remove_project(&self, index: usize) -> Result<(), String> {
        {
            let mut inner = self.inner.write().await;
            if index >= inner.projects.len() {
                return Err("Invalid project index".into());
            }
            if inner.projects.len() <= 1 {
                return Err("Cannot remove the last project".into());
            }
            inner.projects.remove(index);
            // Adjust active project if needed
            if inner.active_project >= inner.projects.len() {
                inner.active_project = inner.projects.len() - 1;
            }
        }

        // Persist to config file
        if let Ok(mut config) = Config::load() {
            if index < config.projects.len() {
                config.projects.remove(index);
                if let Err(e) = config.save() {
                    warn!("Failed to save config after removing project: {e}");
                }
            }
        }

        let _ = self.event_tx.send(WebEvent::StateChanged);
        Ok(())
    }

    /// Select a session within a project (tells the opencode server too).
    pub async fn select_session(&self, project_idx: usize, session_id: String) -> bool {
        let mut inner = self.inner.write().await;
        if let Some(project) = inner.projects.get_mut(project_idx) {
            // Verify session exists
            if project.sessions.iter().any(|s| s.id == session_id) {
                project.active_session = Some(session_id.clone());
                drop(inner); // Release lock before async API call

                // Tell the opencode server about the selection
                let base = base_url().to_string();
                let client = ApiClient::new();
                let dir = {
                    let inner = self.inner.read().await;
                    inner
                        .projects
                        .get(project_idx)
                        .map(|p| p.path.to_string_lossy().to_string())
                        .unwrap_or_default()
                };
                if let Err(e) = client.select_session(&base, &dir, &session_id).await {
                    warn!("Failed to select session via API: {}", e);
                }

                let _ = self.event_tx.send(WebEvent::StateChanged);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Add a new session to a project and activate it immediately.
    pub async fn add_and_activate_session(
        &self,
        project_idx: usize,
        session_info: crate::app::SessionInfo,
    ) {
        let mut inner = self.inner.write().await;
        if let Some(project) = inner.projects.get_mut(project_idx) {
            let session_id = session_info.id.clone();
            // Only add if not already present (avoid duplicates).
            if !project.sessions.iter().any(|s| s.id == session_id) {
                project.sessions.push(session_info);
            }
            project.active_session = Some(session_id);
            let _ = self.event_tx.send(WebEvent::StateChanged);
        }
    }

    /// Toggle a panel's visibility.
    pub async fn toggle_panel(&self, panel: &str) -> bool {
        let mut inner = self.inner.write().await;
        let ok = match panel {
            "Sidebar" | "sidebar" => {
                inner.panels.sidebar = !inner.panels.sidebar;
                true
            }
            "TerminalPane" | "terminal_pane" => {
                inner.panels.terminal_pane = !inner.panels.terminal_pane;
                true
            }
            "NeovimPane" | "neovim_pane" => {
                inner.panels.neovim_pane = !inner.panels.neovim_pane;
                true
            }
            "IntegratedTerminal" | "integrated_terminal" => {
                inner.panels.integrated_terminal = !inner.panels.integrated_terminal;
                true
            }
            "GitPanel" | "git_panel" => {
                inner.panels.git_panel = !inner.panels.git_panel;
                true
            }
            _ => false,
        };
        if ok {
            let _ = self.event_tx.send(WebEvent::StateChanged);
        }
        ok
    }

    /// Set the focused panel.
    pub async fn focus_panel(&self, panel: &str) -> bool {
        let valid = matches!(
            panel,
            "Sidebar"
                | "sidebar"
                | "TerminalPane"
                | "terminal_pane"
                | "NeovimPane"
                | "neovim_pane"
                | "IntegratedTerminal"
                | "integrated_terminal"
                | "GitPanel"
                | "git_panel"
        );
        if valid {
            let mut inner = self.inner.write().await;
            // Normalize to PascalCase
            inner.focused = match panel {
                "sidebar" => "Sidebar",
                "terminal_pane" => "TerminalPane",
                "neovim_pane" => "NeovimPane",
                "integrated_terminal" => "IntegratedTerminal",
                "git_panel" => "GitPanel",
                other => other,
            }
            .to_string();
            let _ = self.event_tx.send(WebEvent::StateChanged);
            true
        } else {
            false
        }
    }

    /// Get the active session ID for the active project.
    pub async fn active_session_id(&self) -> Option<String> {
        let inner = self.inner.read().await;
        inner
            .projects
            .get(inner.active_project)
            .and_then(|p| p.active_session.clone())
    }

    // ── Session Continuity: Presence + Activity ─────────────────────

    /// Register or update a client's presence.
    pub async fn register_presence(&self, req: &ClientPresence) {
        let mut state = self.inner.write().await;
        state.connected_clients.insert(req.client_id.clone(), req.clone());
        let snapshot = PresenceSnapshot {
            clients: state.connected_clients.values().cloned().collect(),
        };
        drop(state);
        let _ = self.event_tx.send(WebEvent::PresenceChanged(snapshot));
    }

    /// Remove a client's presence.
    pub async fn deregister_presence(&self, client_id: &str) {
        let mut state = self.inner.write().await;
        state.connected_clients.remove(client_id);
        let snapshot = PresenceSnapshot {
            clients: state.connected_clients.values().cloned().collect(),
        };
        drop(state);
        let _ = self.event_tx.send(WebEvent::PresenceChanged(snapshot));
    }

    /// Get current presence snapshot.
    pub async fn get_presence(&self) -> PresenceSnapshot {
        let state = self.inner.read().await;
        PresenceSnapshot {
            clients: state.connected_clients.values().cloned().collect(),
        }
    }

    /// Push an activity event for a session (stores + broadcasts).
    pub async fn push_activity_event(&self, event: ActivityEventPayload) {
        let session_id = event.session_id.clone();
        {
            let mut state = self.inner.write().await;
            let log = state.activity_log.entry(session_id).or_default();
            log.push(event.clone());
            // Ring buffer: keep last 200 events per session
            if log.len() > 200 {
                let drain = log.len() - 200;
                log.drain(..drain);
            }
        }
        let _ = self.event_tx.send(WebEvent::ActivityEvent(event));
    }

    /// Get recent activity events for a session.
    pub async fn get_activity_feed(&self, session_id: &str) -> Vec<ActivityEventPayload> {
        let state = self.inner.read().await;
        state
            .activity_log
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    // ── Workspace Snapshots ─────────────────────────────────────────

    /// List all saved missions.
    pub async fn list_missions(&self) -> Vec<Mission> {
        let state = self.inner.read().await;
        let mut list: Vec<Mission> = state.missions.values().cloned().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Create a new mission.
    pub async fn create_mission(&self, req: CreateMissionRequest) -> Mission {
        let now = Utc::now().to_rfc3339();
        let mission = Mission {
            id: format!("mission-{}", uuid_like_id()),
            title: req.title,
            goal: req.goal,
            next_action: req.next_action,
            status: req.status.unwrap_or(MissionStatus::Planned),
            project_index: req.project_index,
            session_id: req.session_id,
            created_at: now.clone(),
            updated_at: now,
        };

        let mut state = self.inner.write().await;
        state.missions.insert(mission.id.clone(), mission.clone());
        drop(state);
        self.schedule_persist();
        mission
    }

    /// Update an existing mission.
    pub async fn update_mission(&self, mission_id: &str, req: UpdateMissionRequest) -> Option<Mission> {
        let mut state = self.inner.write().await;
        let mission = state.missions.get_mut(mission_id)?;

        if let Some(title) = req.title {
            mission.title = title;
        }
        if let Some(goal) = req.goal {
            mission.goal = goal;
        }
        if let Some(next_action) = req.next_action {
            mission.next_action = next_action;
        }
        if let Some(status) = req.status {
            mission.status = status;
        }
        if let Some(project_index) = req.project_index {
            mission.project_index = project_index;
        }
        if let Some(session_id) = req.session_id {
            mission.session_id = session_id;
        }
        mission.updated_at = Utc::now().to_rfc3339();
        let updated = mission.clone();
        drop(state);
        self.schedule_persist();
        Some(updated)
    }

    /// Delete a mission by ID. Returns true if it existed.
    pub async fn delete_mission(&self, mission_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.missions.remove(mission_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }

    /// List all personal memory items.
    pub async fn list_personal_memory(&self) -> Vec<PersonalMemoryItem> {
        let state = self.inner.read().await;
        let mut list: Vec<PersonalMemoryItem> = state.personal_memory.values().cloned().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Create a personal memory item.
    pub async fn create_personal_memory(&self, req: CreatePersonalMemoryRequest) -> PersonalMemoryItem {
        let now = Utc::now().to_rfc3339();
        let item = PersonalMemoryItem {
            id: format!("memory-{}", uuid_like_id()),
            label: req.label,
            content: req.content,
            scope: req.scope,
            project_index: req.project_index,
            session_id: req.session_id,
            created_at: now.clone(),
            updated_at: now,
        };

        let mut state = self.inner.write().await;
        state.personal_memory.insert(item.id.clone(), item.clone());
        drop(state);
        self.schedule_persist();
        item
    }

    /// Update a personal memory item.
    pub async fn update_personal_memory(
        &self,
        memory_id: &str,
        req: UpdatePersonalMemoryRequest,
    ) -> Option<PersonalMemoryItem> {
        let mut state = self.inner.write().await;
        let item = state.personal_memory.get_mut(memory_id)?;

        if let Some(label) = req.label {
            item.label = label;
        }
        if let Some(content) = req.content {
            item.content = content;
        }
        if let Some(scope) = req.scope {
            item.scope = scope;
        }
        if let Some(project_index) = req.project_index {
            item.project_index = project_index;
        }
        if let Some(session_id) = req.session_id {
            item.session_id = session_id;
        }
        item.updated_at = Utc::now().to_rfc3339();
        let updated = item.clone();
        drop(state);
        self.schedule_persist();
        Some(updated)
    }

    /// Delete a personal memory item.
    pub async fn delete_personal_memory(&self, memory_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.personal_memory.remove(memory_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }

    /// Get current autonomy settings.
    pub async fn get_autonomy_settings(&self) -> AutonomySettings {
        let state = self.inner.read().await;
        state.autonomy_settings.clone()
    }

    /// Update autonomy settings.
    pub async fn update_autonomy_settings(&self, mode: AutonomyMode) -> AutonomySettings {
        let mut state = self.inner.write().await;
        state.autonomy_settings = AutonomySettings {
            mode,
            updated_at: Utc::now().to_rfc3339(),
        };
        let settings = state.autonomy_settings.clone();
        drop(state);
        self.schedule_persist();
        settings
    }

    /// List routines and recent runs.
    pub async fn list_routines(&self) -> (Vec<RoutineDefinition>, Vec<RoutineRunRecord>) {
        let state = self.inner.read().await;
        let mut routines: Vec<RoutineDefinition> = state.routines.values().cloned().collect();
        routines.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        let runs = state.routine_runs.clone();
        (routines, runs)
    }

    /// Create a routine.
    pub async fn create_routine(&self, req: CreateRoutineRequest) -> RoutineDefinition {
        let now = Utc::now().to_rfc3339();
        let routine = RoutineDefinition {
            id: format!("routine-{}", uuid_like_id()),
            name: req.name,
            trigger: req.trigger,
            action: req.action,
            mission_id: req.mission_id,
            session_id: req.session_id,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut state = self.inner.write().await;
        state.routines.insert(routine.id.clone(), routine.clone());
        drop(state);
        self.schedule_persist();
        routine
    }

    /// Update a routine.
    pub async fn update_routine(&self, routine_id: &str, req: UpdateRoutineRequest) -> Option<RoutineDefinition> {
        let mut state = self.inner.write().await;
        let routine = state.routines.get_mut(routine_id)?;

        if let Some(name) = req.name {
            routine.name = name;
        }
        if let Some(trigger) = req.trigger {
            routine.trigger = trigger;
        }
        if let Some(action) = req.action {
            routine.action = action;
        }
        if let Some(mission_id) = req.mission_id {
            routine.mission_id = mission_id;
        }
        if let Some(session_id) = req.session_id {
            routine.session_id = session_id;
        }
        routine.updated_at = Utc::now().to_rfc3339();
        let updated = routine.clone();
        drop(state);
        self.schedule_persist();
        Some(updated)
    }

    /// Delete a routine.
    pub async fn delete_routine(&self, routine_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.routines.remove(routine_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }

    /// Record a routine run.
    pub async fn record_routine_run(&self, routine_id: &str, summary: String) -> RoutineRunRecord {
        let run = RoutineRunRecord {
            id: format!("routine-run-{}", uuid_like_id()),
            routine_id: routine_id.to_string(),
            status: "completed".to_string(),
            summary,
            created_at: Utc::now().to_rfc3339(),
        };
        let mut state = self.inner.write().await;
        state.routine_runs.insert(0, run.clone());
        if state.routine_runs.len() > 50 {
            state.routine_runs.truncate(50);
        }
        drop(state);
        self.schedule_persist();
        run
    }

    /// List delegated work items.
    pub async fn list_delegated_work(&self) -> Vec<DelegatedWorkItem> {
        let state = self.inner.read().await;
        let mut list: Vec<DelegatedWorkItem> = state.delegated_work.values().cloned().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Create delegated work item.
    pub async fn create_delegated_work(&self, req: CreateDelegatedWorkRequest) -> DelegatedWorkItem {
        let now = Utc::now().to_rfc3339();
        let item = DelegatedWorkItem {
            id: format!("delegation-{}", uuid_like_id()),
            title: req.title,
            assignee: req.assignee,
            scope: req.scope,
            status: DelegationStatus::Planned,
            mission_id: req.mission_id,
            session_id: req.session_id,
            subagent_session_id: req.subagent_session_id,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut state = self.inner.write().await;
        state.delegated_work.insert(item.id.clone(), item.clone());
        drop(state);
        self.schedule_persist();
        item
    }

    /// Update delegated work item.
    pub async fn update_delegated_work(&self, item_id: &str, req: UpdateDelegatedWorkRequest) -> Option<DelegatedWorkItem> {
        let mut state = self.inner.write().await;
        let item = state.delegated_work.get_mut(item_id)?;
        if let Some(status) = req.status {
            item.status = status;
        }
        item.updated_at = Utc::now().to_rfc3339();
        let updated = item.clone();
        drop(state);
        self.schedule_persist();
        Some(updated)
    }

    /// Delete delegated work item.
    pub async fn delete_delegated_work(&self, item_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.delegated_work.remove(item_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }

    /// List all saved workspace snapshots.
    pub async fn list_workspaces(&self) -> Vec<WorkspaceSnapshot> {
        let state = self.inner.read().await;
        let mut list: Vec<WorkspaceSnapshot> = state.workspaces.values().cloned().collect();
        // Sort by creation time (newest first).
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    /// Save (upsert) a workspace snapshot.
    pub async fn save_workspace(&self, snapshot: WorkspaceSnapshot) {
        let mut state = self.inner.write().await;
        state
            .workspaces
            .insert(snapshot.name.clone(), snapshot);
        drop(state);
        self.schedule_persist();
    }

    /// Delete a workspace snapshot by name. Returns true if it existed.
    pub async fn delete_workspace(&self, name: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.workspaces.remove(name).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }

    fn schedule_persist(&self) {
        let _ = self.persist_tx.send(());
    }

    fn spawn_persist_worker(&self, mut persist_rx: mpsc::UnboundedReceiver<()>) {
        let inner = self.inner.clone();
        let persist_path = self.persist_path.clone();

        tokio::spawn(async move {
            while persist_rx.recv().await.is_some() {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                while persist_rx.try_recv().is_ok() {}

                let state = inner.read().await;
                let payload = PersistedAssistantState {
                    missions: state.missions.clone(),
                    personal_memory: state.personal_memory.clone(),
                    autonomy_settings: Some(state.autonomy_settings.clone()),
                    routines: state.routines.clone(),
                    routine_runs: state.routine_runs.clone(),
                    delegated_work: state.delegated_work.clone(),
                    workspaces: state.workspaces.clone(),
                };
                drop(state);

                let persist_path = persist_path.clone();
                let write_result = tokio::task::spawn_blocking(move || {
                    if let Some(parent) = persist_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let json = serde_json::to_string_pretty(&payload)?;
                    fs::write(&persist_path, json)?;
                    Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
                })
                .await;

                match write_result {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => warn!("failed to persist assistant state: {}", err),
                    Err(err) => warn!("persist worker join error: {}", err),
                }
            }
        });
    }

    // ── Background tasks ────────────────────────────────────────────

    /// Poll `GET /session` for each project every 30 seconds.
    fn spawn_session_poller(&self) {
        let inner = self.inner.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let client = ApiClient::new();

            // Wait for the opencode server to be ready
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            loop {
                let base = base_url().to_string();

                // Snapshot project paths
                let project_paths: Vec<(usize, String)> = {
                    let state = inner.read().await;
                    state
                        .projects
                        .iter()
                        .enumerate()
                        .map(|(i, p)| (i, p.path.to_string_lossy().to_string()))
                        .collect()
                };

                let mut changed = false;

                let fetches = project_paths.iter().map(|(idx, dir)| {
                    let client = &client;
                    let base = base.clone();
                    let dir = dir.clone();
                    let idx = *idx;
                    async move {
                        // Run both fetches concurrently within each project
                        let (sessions, status_map) = tokio::join!(
                            client.fetch_sessions(&base, &dir),
                            client.fetch_session_status(&base, &dir)
                        );
                        (idx, dir, sessions.ok(), status_map.ok())
                    }
                });

                let results = join_all(fetches).await;

                let mut aggregated_busy = HashSet::new();

                for (idx, dir, sessions, status_map) in results {
                    if let Some(sessions) = sessions {
                        let mut state = inner.write().await;
                        if let Some(project) = state.projects.get_mut(idx) {
                            let filtered: Vec<_> = sessions
                                .into_iter()
                                .filter(|s| s.directory == dir)
                                .collect();
                            if project.active_session.is_none() {
                                if let Some(first) = filtered.first() {
                                    project.active_session = Some(first.id.clone());
                                }
                            }
                            // Only mark changed if the session list actually differs
                            let sessions_differ = {
                                if project.sessions.len() != filtered.len() {
                                    true
                                } else {
                                    project.sessions.iter().zip(filtered.iter()).any(|(a, b)| {
                                        a.id != b.id || a.title != b.title || a.time.updated != b.time.updated
                                    })
                                }
                            };
                            if sessions_differ {
                                project.sessions = filtered;
                                changed = true;
                            }
                        }
                    }

                    if let Some(status_map) = status_map {
                        for (session_id, status) in &status_map {
                            if status != "idle" {
                                aggregated_busy.insert(session_id.clone());
                            }
                        }
                    }
                }

                {
                    let mut state = inner.write().await;
                    for id in &aggregated_busy {
                        if !state.busy_sessions.contains(id) {
                            let _ = event_tx.send(WebEvent::SessionBusy {
                                session_id: id.clone(),
                            });
                        }
                    }
                    for id in state.busy_sessions.iter() {
                        if !aggregated_busy.contains(id) {
                            let _ = event_tx.send(WebEvent::SessionIdle {
                                session_id: id.clone(),
                            });
                        }
                    }
                    state.busy_sessions = aggregated_busy;
                }

                if changed {
                    let _ = event_tx.send(WebEvent::StateChanged);
                }

                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });
    }

    /// Listen to the opencode server's SSE `/event` stream to capture
    /// session stats (cost/tokens) from `message.updated` events.
    ///
    /// Spawns one SSE connection per project directory. Every 30 seconds the
    /// connections are torn down and re-established so we pick up any new
    /// projects and drop stale connections.
    fn spawn_opencode_sse_listener(&self) {
        let handle = self.clone();

        tokio::spawn(async move {
            // Wait for server to be ready
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

            loop {
                // Cancel previous SSE tasks before spawning new ones
                for h in handles.drain(..) {
                    h.abort();
                }

                let base = base_url().to_string();

                // Get all project dirs to listen for
                let project_dirs: Vec<String> = {
                    let state = handle.inner.read().await;
                    state
                        .projects
                        .iter()
                        .map(|p| p.path.to_string_lossy().to_string())
                        .collect()
                };

                // Connect SSE for each project
                for dir in &project_dirs {
                    let handle_clone = handle.clone();
                    let dir_clone = dir.clone();
                    let base_clone = base.clone();

                    let h = tokio::spawn(async move {
                        if let Err(e) =
                            run_opencode_sse(&handle_clone, &base_clone, &dir_clone)
                                .await
                        {
                            debug!("OpenCode SSE stream error for {}: {}", dir_clone, e);
                        }
                    });
                    handles.push(h);
                }

                // Reconnect loop: check every 5 minutes if we need to restart
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            }
        });
    }
}

fn uuid_like_id() -> String {
    format!(
        "{:x}{:x}",
        rand::random::<u64>(),
        rand::random::<u64>()
    )
}

fn assistant_state_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("opman")
        .join("web-assistant-state.json")
}

fn load_persisted_assistant_state(path: &PathBuf) -> PersistedAssistantState {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<PersistedAssistantState>(&content) {
            Ok(state) => state,
            Err(err) => {
                warn!("failed to parse persisted assistant state: {}", err);
                PersistedAssistantState::default()
            }
        },
        Err(_) => PersistedAssistantState::default(),
    }
}

// ── SSE stream consumer ─────────────────────────────────────────────

/// Connect to the opencode server's SSE `/event` endpoint and process
/// `message.updated` events to capture session stats.
async fn run_opencode_sse(
    handle: &WebStateHandle,
    base_url: &str,
    project_dir: &str,
) -> anyhow::Result<()> {
    use futures::StreamExt;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/event", base_url))
        .header("Accept", "text/event-stream")
        .header("x-opencode-directory", project_dir)
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("SSE endpoint returned status {}", response.status());
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        // Process complete SSE messages (separated by double newline)
        while let Some(boundary) = buffer.find("\n\n") {
            let message: String = buffer.drain(..boundary).collect();
            buffer.drain(..2); // consume the "\n\n" separator

            if let Some(data) = extract_sse_data(&message) {
                handle_web_sse_event(handle, &data, project_dir).await;
            }
        }
    }

    Ok(())
}

fn extract_sse_data(message: &str) -> Option<String> {
    let mut data_parts = Vec::new();
    for line in message.lines() {
        if let Some(stripped) = line.strip_prefix("data:") {
            data_parts.push(stripped.trim().to_string());
        }
    }
    if data_parts.is_empty() {
        None
    } else {
        Some(data_parts.join("\n"))
    }
}

/// Process a single SSE event from the opencode server.
/// We care about:
/// - `message.updated` → session stats (cost, tokens)
/// - `session.created` → add to sessions list
/// - `session.updated` → update session metadata
/// - `session.deleted` → remove from sessions list
/// - `session.status` → busy/idle tracking + watcher triggers
/// - `file.edited` → file edit tracking for diff review
async fn handle_web_sse_event(
    handle: &WebStateHandle,
    data: &str,
    project_dir: &str,
) {
    #[derive(serde::Deserialize)]
    struct SseEvent {
        #[serde(rename = "type")]
        event_type: String,
        properties: serde_json::Value,
    }

    /// The opencode server wraps events in `{directory, payload}` envelope.
    #[derive(serde::Deserialize)]
    struct SseEnvelope {
        #[allow(dead_code)]
        directory: Option<String>,
        payload: SseEvent,
    }

    // Try parsing as envelope first, then as bare event.
    let event: SseEvent = if let Ok(envelope) = serde_json::from_str::<SseEnvelope>(data) {
        envelope.payload
    } else if let Ok(bare) = serde_json::from_str::<SseEvent>(data) {
        bare
    } else {
        return;
    };

    let inner = &handle.inner;
    let event_tx = &handle.event_tx;

    match event.event_type.as_str() {
        "message.updated" => {
            // Extract cost/token info
            if let Some(info) = event.properties.get("info") {
                let session_id = info
                    .get("sessionID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if session_id.is_empty() {
                    return;
                }

                let cost = info.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let tokens = info
                    .get("tokens")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let input = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
                let output = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
                let reasoning = tokens
                    .get("reasoning")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let cache = tokens
                    .get("cache")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let cache_read = cache.get("read").and_then(|v| v.as_u64()).unwrap_or(0);
                let cache_write = cache.get("write").and_then(|v| v.as_u64()).unwrap_or(0);

                let stats = WebSessionStats {
                    cost,
                    input_tokens: input,
                    output_tokens: output,
                    reasoning_tokens: reasoning,
                    cache_read,
                    cache_write,
                };

                // Update stored stats
                {
                    let mut state = inner.write().await;
                    state
                        .session_stats
                        .insert(session_id.clone(), stats.clone());
                }

                let _ = event_tx.send(WebEvent::StatsUpdated(stats));
            }
        }
        "session.created" => {
            if let Some(info) = event.properties.get("info") {
                if let Ok(session) = serde_json::from_value::<SessionInfo>(info.clone()) {
                    let mut state = inner.write().await;

                    // Track parent→child relationships for watcher suppression
                    if !session.parent_id.is_empty() {
                        state
                            .session_children
                            .entry(session.parent_id.clone())
                            .or_default()
                            .insert(session.id.clone());
                    }

                    // Find the project by directory match
                    let dir = &session.directory;
                    for project in &mut state.projects {
                        if project.path.to_string_lossy() == dir.as_str() || dir.is_empty() {
                            // Avoid duplicates
                            if !project.sessions.iter().any(|s| s.id == session.id) {
                                project.sessions.push(session.clone());
                            }
                            // Auto-activate the new session if none is active
                            // (e.g. after clicking "+" which clears active_session).
                            // Only for top-level sessions; child sessions should not
                            // steal focus from the parent.
                            if project.active_session.is_none()
                                && session.parent_id.is_empty()
                            {
                                project.active_session = Some(session.id.clone());
                            }
                            break;
                        }
                    }
                    let _ = event_tx.send(WebEvent::StateChanged);
                }
            }
        }
        "session.updated" => {
            if let Some(info) = event.properties.get("info") {
                if let Ok(session) = serde_json::from_value::<SessionInfo>(info.clone()) {
                    let mut state = inner.write().await;
                    for project in &mut state.projects {
                        if let Some(existing) =
                            project.sessions.iter_mut().find(|s| s.id == session.id)
                        {
                            *existing = session.clone();
                            break;
                        }
                    }
                    let _ = event_tx.send(WebEvent::StateChanged);
                }
            }
        }
        "session.deleted" => {
            if let Some(session_id) = event.properties.get("sessionID").and_then(|v| v.as_str()) {
                // Clean up watcher if session is deleted
                handle.delete_watcher(session_id).await;

                let mut state = inner.write().await;
                for project in &mut state.projects {
                    project.sessions.retain(|s| s.id != session_id);
                    if project.active_session.as_deref() == Some(session_id) {
                        project.active_session = None;
                    }
                }
                state.session_stats.remove(session_id);
                state.busy_sessions.remove(session_id);

                // Clean up session_children
                state.session_children.remove(session_id);
                for children in state.session_children.values_mut() {
                    children.remove(session_id);
                }

                // Clean up file edit tracking
                state.file_snapshots.remove(session_id);
                state.file_edits.remove(session_id);

                let _ = event_tx.send(WebEvent::StateChanged);
            }
        }
        "session.status" => {
            if let Ok(status_props) =
                serde_json::from_value::<SessionStatusProps>(event.properties.clone())
            {
                let sid = status_props.session_id.clone();
                {
                    let mut state = inner.write().await;
                    match status_props.status.status_type.as_str() {
                        "busy" | "retry" => {
                            if !state.busy_sessions.contains(&sid) {
                                state.busy_sessions.insert(sid.clone());
                                let _ = event_tx.send(WebEvent::SessionBusy {
                                    session_id: sid.clone(),
                                });
                            }
                        }
                        "idle" => {
                            if state.busy_sessions.remove(&sid) {
                                let _ = event_tx.send(WebEvent::SessionIdle {
                                    session_id: sid.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                }

                // Watcher integration: trigger on idle, cancel on busy
                match status_props.status.status_type.as_str() {
                    "idle" => {
                        handle.try_trigger_watcher(&sid).await;
                    }
                    "busy" | "retry" => {
                        handle.cancel_watcher_timer(&sid).await;
                    }
                    _ => {}
                }

                // Activity feed: emit status change
                let summary = match status_props.status.status_type.as_str() {
                    "busy" => "Session started processing".to_string(),
                    "idle" => "Session completed".to_string(),
                    "retry" => "Session retrying".to_string(),
                    other => format!("Session status: {other}"),
                };
                handle.push_activity_event(ActivityEventPayload {
                    session_id: sid,
                    kind: "status".to_string(),
                    summary,
                    detail: Some(status_props.status.status_type.clone()),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                }).await;
            }
        }
        "file.edited" => {
            // Extract file path from properties
            if let Some(file_path) = event.properties.get("file").and_then(|v| v.as_str()) {
                // Determine active session for this project directory
                let session_id = {
                    let state = inner.read().await;
                    state
                        .projects
                        .iter()
                        .find(|p| p.path.to_string_lossy() == project_dir)
                        .and_then(|p| p.active_session.clone())
                };

                if let Some(sid) = session_id {
                    let project_path = std::path::PathBuf::from(project_dir);
                    handle
                        .record_file_edit(&sid, file_path, Some(&project_path))
                        .await;

                    // Activity feed: emit file edit event
                    handle.push_activity_event(ActivityEventPayload {
                        session_id: sid.clone(),
                        kind: "file_edit".to_string(),
                        summary: format!("Edited {file_path}"),
                        detail: Some(file_path.to_string()),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    }).await;

                    // Broadcast a state change so the frontend knows a new file edit happened
                    let _ = event_tx.send(WebEvent::StateChanged);
                }
            }
        }
        _ => {
            // Ignore other event types
        }
    }
}

/// SSE status properties (reused from main SSE module pattern).
#[derive(serde::Deserialize)]
struct SessionStatusProps {
    #[serde(rename = "sessionID")]
    session_id: String,
    status: SessionStatusInfo,
}

#[derive(serde::Deserialize)]
struct SessionStatusInfo {
    #[serde(rename = "type")]
    status_type: String,
}
