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
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{broadcast, RwLock};
use tracing::{debug, warn};

use crate::api::ApiClient;
use crate::app::{base_url, SessionInfo};
use crate::config::Config;

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
}

// ── Public handle ───────────────────────────────────────────────────

/// Async-safe, cloneable handle to the web state. Used by Axum handlers.
#[derive(Clone)]
pub struct WebStateHandle {
    inner: Arc<RwLock<WebStateInner>>,
    /// Broadcast channel for notifying SSE clients of state changes.
    event_tx: broadcast::Sender<WebEvent>,
}

impl WebStateHandle {
    /// Create the web state from config, start background pollers.
    ///
    /// `event_tx` is the broadcast channel that SSE clients subscribe to.
    pub fn new(config: &Config, event_tx: broadcast::Sender<WebEvent>) -> Self {
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
        }));

        let handle = Self { inner, event_tx };

        // Spawn background tasks
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

    /// Get the working directory of the active project.
    pub async fn get_working_dir(&self) -> Option<PathBuf> {
        let inner = self.inner.read().await;
        inner
            .projects
            .get(inner.active_project)
            .map(|p| p.path.clone())
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

    /// Create a new session. The opencode server auto-creates sessions
    /// when `opencode attach` connects, so we just clear the active session
    /// and let the next PTY spawn create a fresh one.
    pub async fn new_session(&self, project_idx: usize) -> bool {
        let mut inner = self.inner.write().await;
        if let Some(project) = inner.projects.get_mut(project_idx) {
            project.active_session = None;
            let _ = self.event_tx.send(WebEvent::StateChanged);
            true
        } else {
            false
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

    // ── Background tasks ────────────────────────────────────────────

    /// Poll `GET /session` for each project every 3 seconds.
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

                for (idx, dir) in &project_paths {
                    // Fetch sessions
                    if let Ok(sessions) = client.fetch_sessions(&base, dir).await {
                        let mut state = inner.write().await;
                        if let Some(project) = state.projects.get_mut(*idx) {
                            // Auto-select first session if none active
                            if project.active_session.is_none() {
                                if let Some(first) = sessions.first() {
                                    project.active_session = Some(first.id.clone());
                                }
                            }
                            project.sessions = sessions;
                            changed = true;
                        }
                    }

                    // Fetch session status (busy/idle)
                    if let Ok(status_map) = client.fetch_session_status(&base, dir).await {
                        let mut state = inner.write().await;
                        let mut new_busy = HashSet::new();
                        for (session_id, status) in &status_map {
                            if status != "idle" {
                                new_busy.insert(session_id.clone());
                            }
                        }

                        // Detect transitions for SSE events
                        for id in &new_busy {
                            if !state.busy_sessions.contains(id) {
                                let _ = event_tx.send(WebEvent::SessionBusy {
                                    session_id: id.clone(),
                                });
                            }
                        }
                        for id in state.busy_sessions.iter() {
                            if !new_busy.contains(id) {
                                let _ = event_tx.send(WebEvent::SessionIdle {
                                    session_id: id.clone(),
                                });
                            }
                        }

                        state.busy_sessions = new_busy;
                        changed = true;
                    }
                }

                if changed {
                    let _ = event_tx.send(WebEvent::StateChanged);
                }

                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
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
        let inner = self.inner.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            // Wait for server to be ready
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

            loop {
                // Cancel previous SSE tasks before spawning new ones
                for handle in handles.drain(..) {
                    handle.abort();
                }

                let base = base_url().to_string();

                // Get all project dirs to listen for
                let project_dirs: Vec<String> = {
                    let state = inner.read().await;
                    state
                        .projects
                        .iter()
                        .map(|p| p.path.to_string_lossy().to_string())
                        .collect()
                };

                // Connect SSE for each project
                for dir in &project_dirs {
                    let inner_clone = inner.clone();
                    let event_tx_clone = event_tx.clone();
                    let dir_clone = dir.clone();
                    let base_clone = base.clone();

                    let handle = tokio::spawn(async move {
                        if let Err(e) =
                            run_opencode_sse(&inner_clone, &event_tx_clone, &base_clone, &dir_clone)
                                .await
                        {
                            debug!("OpenCode SSE stream error for {}: {}", dir_clone, e);
                        }
                    });
                    handles.push(handle);
                }

                // Reconnect loop: check every 30s if we need to restart
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });
    }
}

// ── SSE stream consumer ─────────────────────────────────────────────

/// Connect to the opencode server's SSE `/event` endpoint and process
/// `message.updated` events to capture session stats.
async fn run_opencode_sse(
    inner: &Arc<RwLock<WebStateInner>>,
    event_tx: &broadcast::Sender<WebEvent>,
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
            let message = buffer[..boundary].to_string();
            buffer = buffer[boundary + 2..].to_string();

            if let Some(data) = extract_sse_data(&message) {
                handle_web_sse_event(inner, event_tx, &data).await;
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
/// - `session.status` → busy/idle tracking
async fn handle_web_sse_event(
    inner: &Arc<RwLock<WebStateInner>>,
    event_tx: &broadcast::Sender<WebEvent>,
    data: &str,
) {
    #[derive(serde::Deserialize)]
    struct SseEvent {
        #[serde(rename = "type")]
        event_type: String,
        properties: serde_json::Value,
    }

    let event: SseEvent = match serde_json::from_str(data) {
        Ok(e) => e,
        Err(_) => return,
    };

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
                    // Find the project by directory match
                    let dir = &session.directory;
                    for project in &mut state.projects {
                        if project.path.to_string_lossy() == dir.as_str() || dir.is_empty() {
                            // Avoid duplicates
                            if !project.sessions.iter().any(|s| s.id == session.id) {
                                project.sessions.push(session.clone());
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
                let mut state = inner.write().await;
                for project in &mut state.projects {
                    project.sessions.retain(|s| s.id != session_id);
                    if project.active_session.as_deref() == Some(session_id) {
                        project.active_session = None;
                    }
                }
                state.session_stats.remove(session_id);
                state.busy_sessions.remove(session_id);
                let _ = event_tx.send(WebEvent::StateChanged);
            }
        }
        "session.status" => {
            if let Ok(status_props) =
                serde_json::from_value::<SessionStatusProps>(event.properties.clone())
            {
                let mut state = inner.write().await;
                let sid = &status_props.session_id;
                match status_props.status.status_type.as_str() {
                    "busy" | "retry" => {
                        if !state.busy_sessions.contains(sid) {
                            state.busy_sessions.insert(sid.clone());
                            let _ = event_tx.send(WebEvent::SessionBusy {
                                session_id: sid.clone(),
                            });
                        }
                    }
                    "idle" => {
                        if state.busy_sessions.remove(sid) {
                            let _ = event_tx.send(WebEvent::SessionIdle {
                                session_id: sid.clone(),
                            });
                        }
                    }
                    _ => {}
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
