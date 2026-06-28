//! Embedded opencode-compatible HTTP/SSE adapter, backed by the `claude` CLI.
//!
//! When opman runs with `--claude` (backend = ClaudeCode), this in-process axum
//! server stands in for `opencode serve`. opman's `api/`, `sse/`, `web/`, Slack, and
//! stats layers keep speaking the opencode REST + SSE contract to `base_url`,
//! unaware that the backend is now `claude` background agents.
//!
//! See `registry.rs` (session id mapping), `claude_cli.rs` (process wrappers),
//! `jsonl.rs` (transcript → messages), `events.rs` (transcript → SSE events),
//! `tailer.rs` (live event streaming + busy/idle polling), `routes.rs` (REST).

mod claude_cli;
mod events;
mod jsonl;
mod registry;
mod routes;
mod tailer;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use tokio::sync::{broadcast, oneshot};
use tracing::info;

use crate::server::ServerHandle;
use registry::{Registry, SessionEntry};

/// How opman answered a pending permission/question request from the hook.
#[derive(Debug)]
pub enum PendingReply {
    /// Permission reply: "once" | "always" | "reject".
    Permission(String),
    /// Question answers: one list of selected labels per question.
    Question(Vec<Vec<String>>),
    /// Question/permission was rejected/dismissed.
    Reject,
}

/// A translated opencode event, tagged with the project directory it belongs to so
/// the per-project SSE stream can filter to the right subscriber.
#[derive(Clone, Debug)]
pub struct EngineEvent {
    pub directory: String,
    /// Pre-serialized `{ "type": …, "properties": … }` JSON.
    pub data: String,
}

/// The embedded Claude engine: session registry + event bus.
pub struct ClaudeEngine {
    reg: Mutex<Registry>,
    persist: Option<PathBuf>,
    events: broadcast::Sender<EngineEvent>,
    tailers: Mutex<HashSet<String>>,
    /// In-flight permission/question requests awaiting an opman reply.
    pending: Mutex<HashMap<String, oneshot::Sender<PendingReply>>>,
    /// Engine HTTP base URL (set once bound); passed to hook subprocesses.
    url: Mutex<String>,
    /// Default permission mode when a session has no override.
    default_mode: String,
    /// Path to the opman executable (used as the PreToolUse hook command).
    exe: PathBuf,
}

static ENGINE: OnceLock<Arc<ClaudeEngine>> = OnceLock::new();

/// Global accessor (used by the PTY layer to resolve a session's `claude` short id).
pub fn engine() -> Option<Arc<ClaudeEngine>> {
    ENGINE.get().cloned()
}

/// The current `claude` background short id for a session (for `claude attach`).
pub fn short_id_for_session(session_id: &str) -> Option<String> {
    engine()
        .and_then(|e| e.get_session(session_id))
        .and_then(|s| s.short_id)
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn rand_id(prefix: &str) -> String {
    let n: u128 = rand::random();
    format!("{prefix}_{n:032x}")
}

impl ClaudeEngine {
    fn new(persist: Option<PathBuf>) -> Self {
        let reg = match &persist {
            Some(p) => Registry::load(p),
            None => Registry::default(),
        };
        let (events, _) = broadcast::channel(2048);
        // Default to bypassPermissions (no per-tool prompts) — `AskUserQuestion`
        // still surfaces to opman regardless of mode, and the user can switch to a
        // prompting mode at runtime via `/permission-mode <mode>`.
        let default_mode = std::env::var("OPMAN_CLAUDE_PERMISSION_MODE")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "bypassPermissions".to_string());
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("opman"));
        Self {
            reg: Mutex::new(reg),
            persist,
            events,
            tailers: Mutex::new(HashSet::new()),
            pending: Mutex::new(HashMap::new()),
            url: Mutex::new(String::new()),
            default_mode,
            exe,
        }
    }

    fn set_url(&self, url: &str) {
        if let Ok(mut u) = self.url.lock() {
            *u = url.to_string();
        }
    }

    pub fn url(&self) -> String {
        self.url.lock().map(|u| u.clone()).unwrap_or_default()
    }

    pub fn exe(&self) -> PathBuf {
        self.exe.clone()
    }

    /// Effective permission mode for a session (override → engine default).
    pub fn effective_mode(&self, session_id: &str) -> String {
        self.get_session(session_id)
            .and_then(|s| s.permission_mode)
            .unwrap_or_else(|| self.default_mode.clone())
    }

    /// Set a session's permission mode at runtime; returns the new mode.
    pub fn set_permission_mode(&self, session_id: &str, mode: &str) {
        let dir = {
            let mut g = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(e) = g.sessions.get_mut(session_id) else {
                return;
            };
            e.permission_mode = Some(mode.to_string());
            e.directory.clone()
        };
        self.save();
        // Surface a toast so TUI/web confirm the change.
        self.emit(
            &dir,
            "tui.toast.show",
            serde_json::json!({ "message": format!("Claude permission mode: {mode}"), "variant": "info" }),
        );
    }

    fn add_allowed_tool(&self, session_id: &str, tool: &str) {
        if let Ok(mut g) = self.reg.lock() {
            if let Some(e) = g.sessions.get_mut(session_id) {
                if !e.allowed_tools.iter().any(|t| t == tool) {
                    e.allowed_tools.push(tool.to_string());
                }
            }
        }
        self.save();
    }

    fn is_always_allowed(&self, session_id: &str, tool: &str) -> bool {
        self.get_session(session_id)
            .map(|s| s.allowed_tools.iter().any(|t| t == tool))
            .unwrap_or(false)
    }

    /// Register a pending hook request; returns the receiver to await opman's reply.
    pub fn register_pending(&self, id: &str) -> oneshot::Receiver<PendingReply> {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut p) = self.pending.lock() {
            p.insert(id.to_string(), tx);
        }
        rx
    }

    /// Resolve a pending request (called by the reply endpoints). Returns false if
    /// the id was unknown (already resolved or expired).
    pub fn resolve_pending(&self, id: &str, reply: PendingReply) -> bool {
        let tx = self.pending.lock().ok().and_then(|mut p| p.remove(id));
        match tx {
            Some(tx) => tx.send(reply).is_ok(),
            None => false,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    fn save(&self) {
        if let Some(p) = &self.persist {
            if let Ok(reg) = self.reg.lock() {
                reg.save(p);
            }
        }
    }

    /// Emit an opencode-shaped event for `{type, properties}` scoped to `directory`.
    pub fn emit(&self, directory: &str, event_type: &str, properties: serde_json::Value) {
        let payload = serde_json::json!({ "type": event_type, "properties": properties });
        let _ = self.events.send(EngineEvent {
            directory: directory.to_string(),
            data: payload.to_string(),
        });
    }

    pub fn get_session(&self, id: &str) -> Option<SessionEntry> {
        self.reg.lock().ok()?.sessions.get(id).cloned()
    }

    pub fn list_for_dir(&self, dir: &str) -> Vec<SessionEntry> {
        self.reg
            .lock()
            .map(|r| r.for_directory(dir))
            .unwrap_or_default()
    }

    /// Find the opman session whose latest claude UUID matches, returning its id.
    #[allow(dead_code)] // available for future event-routing by claude uuid
    pub fn session_id_for_claude_uuid(&self, uuid: &str) -> Option<String> {
        self.reg
            .lock()
            .ok()?
            .by_claude_uuid(uuid)
            .map(|s| s.id.clone())
    }

    /// Mint a new opman session and announce it via `session.created`.
    pub fn create_session(&self, dir: &str, parent_id: &str, title: &str) -> SessionEntry {
        let now = now_ms();
        let entry = SessionEntry {
            id: rand_id("ses"),
            title: title.to_string(),
            directory: dir.to_string(),
            parent_id: parent_id.to_string(),
            created: now,
            updated: now,
            ..Default::default()
        };
        if let Ok(mut r) = self.reg.lock() {
            r.sessions.insert(entry.id.clone(), entry.clone());
        }
        self.save();
        self.emit(dir, "session.created", serde_json::json!({ "info": session_info(&entry) }));
        entry
    }

    /// Record a new background turn for a session (after `bg_start`/`bg_resume`).
    pub fn record_turn(
        self: &Arc<Self>,
        session_id: &str,
        short_id: String,
        claude_uuid: String,
        model: Option<String>,
    ) {
        let dir = {
            let mut guard = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(entry) = guard.sessions.get_mut(session_id) else {
                return;
            };
            entry.short_id = Some(short_id);
            if !claude_uuid.is_empty() {
                entry.claude_session_id = Some(claude_uuid.clone());
                if entry.lineage.last().map(|s| s.as_str()) != Some(claude_uuid.as_str()) {
                    entry.lineage.push(claude_uuid);
                }
            }
            if model.is_some() {
                entry.model = model;
            }
            entry.busy = true;
            entry.updated = now_ms();
            entry.directory.clone()
        };
        self.save();
        if let Some(entry) = self.get_session(session_id) {
            self.emit(&dir, "session.updated", serde_json::json!({ "info": session_info(&entry) }));
            self.emit(
                &dir,
                "session.status",
                serde_json::json!({ "sessionID": session_id, "status": { "type": "busy" } }),
            );
        }
        self.clone().ensure_tailer(session_id);
    }

    /// Update a session's title (from claude's `ai-title`) and announce it.
    pub fn set_title(&self, session_id: &str, title: &str) {
        let dir = {
            let mut guard = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(entry) = guard.sessions.get_mut(session_id) else {
                return;
            };
            if entry.title == title {
                return;
            }
            entry.title = title.to_string();
            entry.updated = now_ms();
            entry.directory.clone()
        };
        self.save();
        if let Some(entry) = self.get_session(session_id) {
            self.emit(&dir, "session.updated", serde_json::json!({ "info": session_info(&entry) }));
        }
    }

    /// Set busy state; on a transition emit `session.status` (+ `session.idle`).
    pub fn set_busy(&self, session_id: &str, busy: bool) {
        let (dir, changed) = {
            let mut guard = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(entry) = guard.sessions.get_mut(session_id) else {
                return;
            };
            let changed = entry.busy != busy;
            entry.busy = busy;
            (entry.directory.clone(), changed)
        };
        if !changed {
            return;
        }
        let status = if busy { "busy" } else { "idle" };
        self.emit(
            &dir,
            "session.status",
            serde_json::json!({ "sessionID": session_id, "status": { "type": status } }),
        );
        if !busy {
            self.emit(&dir, "session.idle", serde_json::json!({ "sessionID": session_id }));
        }
    }

    /// Map of session id → busy, for `GET /session/status`.
    pub fn busy_map(&self) -> std::collections::HashMap<String, bool> {
        self.reg
            .lock()
            .map(|r| r.sessions.values().map(|s| (s.id.clone(), s.busy)).collect())
            .unwrap_or_default()
    }

    fn ensure_tailer(self: Arc<Self>, session_id: &str) {
        {
            let mut t = match self.tailers.lock() {
                Ok(t) => t,
                Err(_) => return,
            };
            if !t.insert(session_id.to_string()) {
                return; // already running
            }
        }
        tailer::spawn_tailer(self, session_id.to_string());
    }
}

/// PreToolUse hook entry point (`opman claude-hook`).
///
/// Reads the hook payload from stdin, relays it to the running engine's
/// `/internal/ask` endpoint (resolved from `OPMAN_ENGINE_URL`), and prints the
/// permission-decision JSON the `claude` agent expects. Fails open (allow) on any
/// error so a hiccup never wedges an agent.
pub async fn run_permission_hook() -> Result<()> {
    use std::io::Read;

    let allow = || {
        println!(
            "{}",
            serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow"
                }
            })
        );
    };

    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        allow();
        return Ok(());
    }
    let payload: serde_json::Value = serde_json::from_str(&input).unwrap_or(serde_json::Value::Null);

    let url = match std::env::var("OPMAN_ENGINE_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => {
            allow();
            return Ok(());
        }
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{url}/internal/ask"))
        .json(&payload)
        // Generous: a human may take a while to answer a permission/question prompt.
        .timeout(std::time::Duration::from_secs(3600))
        .send()
        .await;

    match resp {
        Ok(r) => match r.text().await {
            Ok(body) if !body.trim().is_empty() => println!("{body}"),
            _ => allow(),
        },
        Err(_) => allow(),
    }
    Ok(())
}

/// Build the opencode `session.info` object opman/web expect.
fn session_info(entry: &SessionEntry) -> serde_json::Value {
    serde_json::json!({
        "id": entry.id,
        "title": entry.title,
        "parentID": entry.parent_id,
        "directory": entry.directory,
        "time": { "created": entry.created, "updated": entry.updated },
    })
}

/// Start the embedded adapter server. Mirrors `server::spawn_agent_server`'s
/// `(base_url, ServerHandle)` return so `main.rs` can swap cleanly.
pub async fn start_embedded_server() -> Result<(String, ServerHandle)> {
    let persist = dirs::config_dir().map(|d| d.join("opman").join("claude_sessions.json"));
    let engine = Arc::new(ClaudeEngine::new(persist));
    let _ = ENGINE.set(engine.clone());

    // Background poller: reconcile busy/idle from `claude agents --json`.
    tailer::spawn_status_poller(engine.clone());

    let app = routes::router(engine.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to bind embedded claude engine port")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}");
    engine.set_url(&url);
    info!(%url, "Claude engine (embedded opencode adapter) ready");

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Claude engine server error: {e}");
        }
    });

    // No external child process to manage; shutdown is implicit on exit.
    let handle: ServerHandle = Arc::new(std::sync::Mutex::new(None));
    Ok((url, handle))
}
