//! Embedded engine backed by `claude -p` streaming print mode.
//!
//! Unlike the background-agent engine (`claude_engine`, `--claude`), this keeps ONE
//! long-lived `claude -p --input-format stream-json` process per opman session. The
//! process is a persistent read-eval loop over newline-delimited user messages on
//! stdin, so a follow-up is *pushed straight to the running model* (true steering — no
//! queue-and-wait), and abort *hard-kills* the process. It speaks the same
//! opencode-compatible REST + SSE contract as `claude_engine`, reusing the shared
//! transcript parser ([`crate::claude_engine::jsonl`]) for message rendering.

mod process;
mod routes;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tracing::info;

use crate::claude_engine::{claude_cli, EngineEvent, PendingReply};
use crate::server::ServerHandle;

pub use process::ProcMap;

static ENGINE: OnceLock<Arc<ClaudePEngine>> = OnceLock::new();

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

fn default_model() -> Option<String> {
    std::env::var("OPMAN_CLAUDE_MODEL").ok().filter(|s| !s.is_empty())
}

/// One opman session backed by a live `claude -p` process.
#[derive(Clone, Default)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub parent_id: String,
    pub created: u64,
    pub updated: u64,
    /// claude session UUID for the live process (from its `system/init` event); used
    /// to locate the on-disk transcript JSONL for rendering.
    pub claude_uuid: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub permission_mode: Option<String>,
    pub allowed_tools: Vec<String>,
    pub busy: bool,
    pub title_locked: bool,
}

/// opencode-shaped session object for REST responses + events.
pub fn session_info(s: &Session) -> Value {
    json!({
        "id": s.id,
        "slug": "",
        "title": s.title,
        "version": claude_cli::version(),
        "projectID": "claude",
        "parentID": s.parent_id,
        "directory": s.directory,
        "time": { "created": s.created, "updated": s.updated },
    })
}

pub struct ClaudePEngine {
    sessions: Mutex<HashMap<String, Session>>,
    events: broadcast::Sender<EngineEvent>,
    /// Live `claude -p` processes, keyed by opman session id.
    procs: ProcMap,
    /// In-flight permission/question requests awaiting an opman reply.
    pending: Mutex<HashMap<String, tokio::sync::oneshot::Sender<PendingReply>>>,
    /// Per-(session,message) content hash, so the stream reader emits a message only
    /// when its rendered content actually changed.
    emitted: Mutex<HashMap<String, u64>>,
    persist: Option<PathBuf>,
    url: Mutex<String>,
    default_mode: String,
    exe: PathBuf,
    command_cache: Mutex<HashMap<String, claude_cli::InitInfo>>,
    mcp_flags: (bool, bool, bool, bool),
}

impl ClaudePEngine {
    fn new(persist: Option<PathBuf>, mcp_flags: (bool, bool, bool, bool)) -> Self {
        let (events, _) = broadcast::channel(2048);
        let default_mode = std::env::var("OPMAN_CLAUDE_PERMISSION_MODE")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "bypassPermissions".to_string());
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("opman"));
        Self {
            sessions: Mutex::new(load_sessions(&persist)),
            events,
            procs: ProcMap::default(),
            pending: Mutex::new(HashMap::new()),
            emitted: Mutex::new(HashMap::new()),
            persist,
            url: Mutex::new(String::new()),
            default_mode,
            exe,
            command_cache: Mutex::new(HashMap::new()),
            mcp_flags,
        }
    }

    // ── event bus ────────────────────────────────────────────────────
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    pub fn emit(&self, directory: &str, event_type: &str, properties: Value) {
        let payload = json!({ "type": event_type, "properties": properties });
        let _ = self.events.send(EngineEvent {
            directory: directory.to_string(),
            data: payload.to_string(),
        });
    }

    // ── registry ─────────────────────────────────────────────────────
    pub fn get_session(&self, id: &str) -> Option<Session> {
        self.sessions.lock().ok()?.get(id).cloned()
    }

    pub fn list_for_dir(&self, dir: &str) -> Vec<Session> {
        let mut v: Vec<Session> = self
            .sessions
            .lock()
            .map(|s| s.values().filter(|x| x.directory == dir).cloned().collect())
            .unwrap_or_default();
        v.sort_by(|a, b| b.created.cmp(&a.created));
        v
    }

    /// session id → busy, for `GET /session/status`.
    pub fn busy_map(&self) -> HashMap<String, bool> {
        self.sessions
            .lock()
            .map(|s| s.values().map(|x| (x.id.clone(), x.busy)).collect())
            .unwrap_or_default()
    }

    pub fn session_id_for_claude_uuid(&self, uuid: &str) -> Option<String> {
        if uuid.is_empty() {
            return None;
        }
        let s = self.sessions.lock().ok()?;
        s.values()
            .find(|x| x.claude_uuid.as_deref() == Some(uuid))
            .map(|x| x.id.clone())
    }

    pub fn create_session(&self, dir: &str, parent_id: &str, title: &str) -> Session {
        let now = now_ms();
        let entry = Session {
            id: rand_id("ses"),
            title: title.to_string(),
            directory: dir.to_string(),
            parent_id: parent_id.to_string(),
            created: now,
            updated: now,
            ..Default::default()
        };
        if let Ok(mut s) = self.sessions.lock() {
            s.insert(entry.id.clone(), entry.clone());
        }
        self.save();
        self.emit(dir, "session.created", json!({ "info": session_info(&entry) }));
        entry
    }

    pub fn rename_session(&self, id: &str, title: &str) {
        let dir = self.mutate(id, |e| {
            e.title = title.to_string();
            e.title_locked = true;
            e.updated = now_ms();
        });
        if let (Some(dir), Some(e)) = (dir, self.get_session(id)) {
            self.emit(&dir, "session.updated", json!({ "info": session_info(&e) }));
        }
    }

    pub async fn delete_session(self: &Arc<Self>, id: &str) {
        process::abort(self.clone(), id).await;
        let dir = self.sessions.lock().ok().and_then(|mut s| s.remove(id)).map(|e| e.directory);
        self.save();
        if let Some(dir) = dir {
            self.emit(&dir, "session.deleted", json!({ "sessionID": id, "id": id }));
        }
    }

    pub fn set_title(&self, id: &str, title: &str, manual: bool) {
        let dir = self.mutate(id, |e| {
            if !manual && e.title_locked {
                return;
            }
            e.title = title.to_string();
            if manual {
                e.title_locked = true;
            }
            e.updated = now_ms();
        });
        if let (Some(dir), Some(e)) = (dir, self.get_session(id)) {
            self.emit(&dir, "session.updated", json!({ "info": session_info(&e) }));
        }
    }

    pub fn set_claude_uuid(&self, id: &str, uuid: &str) {
        self.mutate(id, |e| e.claude_uuid = Some(uuid.to_string()));
        self.save();
    }

    pub fn set_model(&self, id: &str, model: &str) {
        if model.is_empty() {
            return;
        }
        self.mutate(id, |e| e.model = Some(model.to_string()));
        self.save();
    }

    pub fn set_agent(&self, id: &str, agent: &str) {
        let resolved = self.resolve_agent(id, agent);
        self.mutate(id, |e| e.agent = (!resolved.is_empty()).then(|| resolved.clone()));
        self.save();
    }

    pub fn set_permission_mode(&self, id: &str, mode: &str) {
        let dir = self.mutate(id, |e| e.permission_mode = Some(mode.to_string()));
        self.save();
        if let Some(dir) = dir {
            self.emit(
                &dir,
                "tui.toast.show",
                json!({ "message": format!("Claude permission mode: {mode}"), "variant": "info" }),
            );
        }
    }

    /// Set busy and emit status; returns true on a busy→idle edge.
    pub fn set_busy(&self, id: &str, busy: bool) -> bool {
        let (dir, changed) = {
            let mut g = match self.sessions.lock() {
                Ok(g) => g,
                Err(_) => return false,
            };
            let Some(e) = g.sessions_get_mut(id) else { return false };
            let changed = e.busy != busy;
            e.busy = busy;
            (e.directory.clone(), changed)
        };
        if !changed {
            return false;
        }
        let status = if busy { "busy" } else { "idle" };
        self.emit(&dir, "session.status", json!({ "sessionID": id, "status": { "type": status } }));
        if !busy {
            self.emit(&dir, "session.idle", json!({ "sessionID": id }));
        }
        !busy
    }

    pub fn effective_mode(&self, id: &str) -> String {
        self.get_session(id)
            .and_then(|s| s.permission_mode)
            .unwrap_or_else(|| self.default_mode.clone())
    }

    pub fn add_allowed_tool(&self, id: &str, tool: &str) {
        self.mutate(id, |e| {
            if !e.allowed_tools.iter().any(|t| t == tool) {
                e.allowed_tools.push(tool.to_string());
            }
        });
        self.save();
    }

    pub fn is_always_allowed(&self, id: &str, tool: &str) -> bool {
        self.get_session(id)
            .map(|s| s.allowed_tools.iter().any(|t| t == tool))
            .unwrap_or(false)
    }

    // ── pending hook requests ────────────────────────────────────────
    pub fn register_pending(&self, id: &str) -> tokio::sync::oneshot::Receiver<PendingReply> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Ok(mut p) = self.pending.lock() {
            p.insert(id.to_string(), tx);
        }
        rx
    }

    pub fn resolve_pending(&self, id: &str, reply: PendingReply) -> bool {
        let tx = self.pending.lock().ok().and_then(|mut p| p.remove(id));
        tx.map(|tx| tx.send(reply).is_ok()).unwrap_or(false)
    }

    // ── init introspection cache (slash commands + agents) ───────────
    pub fn cached_init(&self, dir: &str) -> Option<claude_cli::InitInfo> {
        self.command_cache.lock().ok()?.get(dir).cloned()
    }

    pub fn set_cached_init(&self, dir: &str, info: claude_cli::InitInfo) {
        if let Ok(mut c) = self.command_cache.lock() {
            c.insert(dir.to_string(), info);
        }
    }

    fn resolve_agent(&self, id: &str, agent: &str) -> String {
        let agent = agent.trim();
        if agent.is_empty() {
            return String::new();
        }
        let dir = self.get_session(id).map(|s| s.directory).unwrap_or_default();
        let known = self.cached_init(&dir).map(|i| i.agents).unwrap_or_default();
        let find = |name: &str| known.iter().find(|a| a.eq_ignore_ascii_case(name)).cloned();
        if let Some(real) = find(agent) {
            return real;
        }
        match agent.to_ascii_lowercase().as_str() {
            "plan" => find("Plan").unwrap_or_else(|| {
                if known.is_empty() { "Plan".to_string() } else { String::new() }
            }),
            "build" | "code-reviewer" | "reviewer" => String::new(),
            _ => {
                if known.is_empty() { agent.to_string() } else { String::new() }
            }
        }
    }

    // ── turn options for `claude -p` ─────────────────────────────────
    pub fn url(&self) -> String {
        self.url.lock().map(|u| u.clone()).unwrap_or_default()
    }
    fn set_url(&self, url: &str) {
        if let Ok(mut u) = self.url.lock() {
            *u = url.to_string();
        }
    }

    fn hook_settings(&self) -> String {
        let cmd = format!("{} claude-hook", self.exe.to_string_lossy());
        json!({
            "hooks": { "PreToolUse": [ { "matcher": "*", "hooks": [ { "type": "command", "command": cmd } ] } ] }
        })
        .to_string()
    }

    fn mcp_config_json(&self, dir: &str, session_id: &str) -> Option<String> {
        let (terminal, neovim, time, ui) = self.mcp_flags;
        if !(terminal || neovim || time || ui) {
            return None;
        }
        let exe = self.exe.to_string_lossy().to_string();
        let env = json!({ "OPENCODE_SESSION_ID": session_id });
        let mut servers = serde_json::Map::new();
        if terminal {
            servers.insert("terminal".into(), json!({ "command": exe, "args": ["mcp", dir], "env": env }));
        }
        if neovim {
            servers.insert("neovim".into(), json!({ "command": exe, "args": ["mcp-nvim", dir], "env": env }));
        }
        if time {
            servers.insert("time".into(), json!({ "command": exe, "args": ["mcp-time"] }));
        }
        if ui {
            servers.insert("ui".into(), json!({ "command": exe, "args": ["mcp-ui"] }));
        }
        Some(json!({ "mcpServers": servers }).to_string())
    }

    /// Resolved options for a session's `claude -p` process.
    fn turn_opts(&self, session_id: &str, dir: &str) -> process::TurnOpts {
        let s = self.get_session(session_id);
        process::TurnOpts {
            model: s.as_ref().and_then(|s| s.model.clone()).or_else(default_model),
            agent: s
                .as_ref()
                .and_then(|s| s.agent.clone())
                .map(|a| self.resolve_agent(session_id, &a))
                .filter(|a| !a.is_empty()),
            permission_mode: self.effective_mode(session_id),
            settings_json: self.hook_settings(),
            engine_url: self.url(),
            mcp_config: self.mcp_config_json(dir, session_id).unwrap_or_default(),
            session_env_id: session_id.to_string(),
        }
    }

    /// Content-hash gate for the stream reader: true if this message's content changed
    /// since the last emit (so we don't re-emit unchanged messages on every line).
    pub fn should_emit(&self, session_id: &str, msg_id: &str, hash: u64) -> bool {
        let key = format!("{session_id}:{msg_id}");
        let mut g = match self.emitted.lock() {
            Ok(g) => g,
            Err(_) => return true,
        };
        if g.get(&key) == Some(&hash) {
            return false;
        }
        g.insert(key, hash);
        true
    }

    // ── helpers ──────────────────────────────────────────────────────
    fn mutate(&self, id: &str, f: impl FnOnce(&mut Session)) -> Option<String> {
        let mut g = self.sessions.lock().ok()?;
        let e = g.get_mut(id)?;
        f(e);
        Some(e.directory.clone())
    }

    fn save(&self) {
        let Some(path) = &self.persist else { return };
        let snapshot: Vec<PersistSession> = self
            .sessions
            .lock()
            .map(|s| s.values().map(PersistSession::from).collect())
            .unwrap_or_default();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(s) = serde_json::to_string_pretty(&snapshot) {
            let _ = std::fs::write(path, s);
        }
    }
}

// Small shim so set_busy can use a single lock guard ergonomically.
trait SessionsGet {
    fn sessions_get_mut(&mut self, id: &str) -> Option<&mut Session>;
}
impl SessionsGet for HashMap<String, Session> {
    fn sessions_get_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.get_mut(id)
    }
}

// ── persistence (titles/config survive restart; live processes do not) ──
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistSession {
    id: String,
    title: String,
    directory: String,
    parent_id: String,
    created: u64,
    updated: u64,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    title_locked: bool,
}

impl From<&Session> for PersistSession {
    fn from(s: &Session) -> Self {
        PersistSession {
            id: s.id.clone(),
            title: s.title.clone(),
            directory: s.directory.clone(),
            parent_id: s.parent_id.clone(),
            created: s.created,
            updated: s.updated,
            model: s.model.clone(),
            agent: s.agent.clone(),
            permission_mode: s.permission_mode.clone(),
            title_locked: s.title_locked,
        }
    }
}

fn load_sessions(persist: &Option<PathBuf>) -> HashMap<String, Session> {
    let Some(path) = persist else { return HashMap::new() };
    let Ok(raw) = std::fs::read_to_string(path) else { return HashMap::new() };
    let list: Vec<PersistSession> = serde_json::from_str(&raw).unwrap_or_default();
    list.into_iter()
        .map(|p| {
            (
                p.id.clone(),
                Session {
                    id: p.id,
                    title: p.title,
                    directory: p.directory,
                    parent_id: p.parent_id,
                    created: p.created,
                    updated: p.updated,
                    claude_uuid: None,
                    model: p.model,
                    agent: p.agent,
                    permission_mode: p.permission_mode,
                    allowed_tools: Vec::new(),
                    busy: false,
                    title_locked: p.title_locked,
                },
            )
        })
        .collect()
}

/// Start the embedded `claude -p` engine. Mirrors `claude_engine::start_embedded_server`
/// so `main.rs` can swap cleanly: returns `(base_url, ServerHandle)`.
pub async fn start_embedded_server(
    mcp_flags: (bool, bool, bool, bool),
) -> Result<(String, ServerHandle)> {
    let persist = dirs::config_dir().map(|d| d.join("opman").join("claude_p_sessions.json"));
    let engine = Arc::new(ClaudePEngine::new(persist, mcp_flags));
    let _ = ENGINE.set(engine.clone());

    let app = routes::router(engine.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to bind embedded claude -p engine port")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}");
    engine.set_url(&url);
    info!(%url, "Claude -p engine (streaming adapter) ready");

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Claude -p engine server error: {e}");
        }
    });

    let handle: ServerHandle = Arc::new(std::sync::Mutex::new(None));
    Ok((url, handle))
}
