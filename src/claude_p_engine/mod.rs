//! Embedded engine backed by `claude -p` streaming print mode.
//!
//! Unlike the background-agent engine (`claude_engine`, `--claude`), this keeps ONE
//! long-lived `claude -p --input-format stream-json` process per opman session. The
//! process is a persistent read-eval loop over newline-delimited user messages on
//! stdin, so a follow-up is *pushed straight to the running model* (true steering — no
//! queue-and-wait), and abort *hard-kills* the process. Conversation continuity across
//! abort/restart is preserved by `--resume <uuid>` (the session's claude UUID is
//! persisted). It speaks the same opencode REST + SSE contract as `claude_engine`,
//! reusing the shared transcript parser ([`crate::claude_engine::jsonl`]) for rendering.

mod dispatch;
mod opts;
mod process;
mod routes;
mod routes_hook;
mod routes_meta;
mod session;
mod state;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tracing::info;

use crate::claude_engine::{claude_cli, EngineEvent};
use crate::server::ServerHandle;

pub use process::ProcMap;
pub use session::{session_info, Session};

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

/// Engine-wide default model (`OPMAN_CLAUDE_MODEL`), if set.
fn default_model() -> Option<String> {
    std::env::var("OPMAN_CLAUDE_MODEL").ok().filter(|s| !s.is_empty())
}

pub struct ClaudePEngine {
    sessions: Mutex<HashMap<String, Session>>,
    events: broadcast::Sender<EngineEvent>,
    /// Live `claude -p` processes, keyed by opman session id.
    procs: ProcMap,
    /// In-flight permission/question requests awaiting an opman reply.
    pending: Mutex<HashMap<String, tokio::sync::oneshot::Sender<crate::claude_engine::PendingReply>>>,
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
            sessions: Mutex::new(session::load_sessions(&persist)),
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

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    /// Emit an opencode-shaped `{type, properties}` event scoped to `directory`.
    pub fn emit(&self, directory: &str, event_type: &str, properties: Value) {
        let payload = json!({ "type": event_type, "properties": properties });
        let _ = self.events.send(EngineEvent {
            directory: directory.to_string(),
            data: payload.to_string(),
        });
    }
}

/// Single-guard accessor used by `set_busy`.
trait SessionsGet {
    fn sessions_get_mut(&mut self, id: &str) -> Option<&mut Session>;
}
impl SessionsGet for HashMap<String, Session> {
    fn sessions_get_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.get_mut(id)
    }
}

/// Start the embedded `claude -p` engine. Mirrors `claude_engine::start_embedded_server`
/// so `main.rs` can swap cleanly: returns `(base_url, ServerHandle)`.
pub async fn start_embedded_server(
    mcp_flags: (bool, bool, bool, bool),
) -> Result<(String, ServerHandle)> {
    let persist = dirs::config_dir().map(|d| d.join("opman").join("claude_p_sessions.json"));
    let engine = Arc::new(ClaudePEngine::new(persist, mcp_flags));

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

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;

#[cfg(test)]
#[path = "mod_env_config_tests.rs"]
mod mod_env_config_tests;
