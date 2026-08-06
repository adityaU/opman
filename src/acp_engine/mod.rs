//! A generic Agent Client Protocol engine.
//!
//! This replaces opman's `claude -p` adapter. The old engine was shaped by one CLI's
//! print mode: it parsed that CLI's stream-json frames, re-read the transcript files that
//! CLI wrote, and gated permissions through a hook that CLI supported. None of it
//! transferred to another agent.
//!
//! Here the engine speaks only [ACP](https://agentclientprotocol.com), and every
//! agent-specific fact lives in [`config`] — so adding an ACP server is a config entry, not
//! a module. Claude is simply the entry that ships by default.
//!
//! Streaming is the other reason for the change: ACP delivers `agent_message_chunk`
//! notifications per token, which opman forwards as `message.part.delta`. Text appears as
//! the model produces it, and a token costs one append instead of a transcript re-parse.
//!
//! Module map: [`jsonrpc`] transport, [`conn`] lifecycle, [`client`] the agent→opman half,
//! [`render`]/[`transcript`]/[`tool`] rendering, [`options`] capability discovery,
//! [`routes`] the opencode-compatible REST + SSE surface.

mod attach;
mod choices;
mod client;
pub mod config;
mod conn;
mod conn_options;
mod discovered;
mod emit;
mod history;
mod jsonrpc;
mod mcp_servers;
mod options;
mod render;
mod routes;
mod routes_meta;
mod routes_turn;
mod session;
mod state;
mod tool;
mod transcript;
mod turn_state;
mod turn;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tracing::info;

use crate::claude_engine::EngineEvent;
use crate::server::ServerHandle;
use config::AgentConfig;
use transcript::Transcript;

pub use conn::ConnMap;
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

/// Per-session state that is derived from the agent rather than chosen by the user, and so
/// is rebuilt on every connection instead of persisted.
#[derive(Default)]
struct Discovered {
    /// The `session/new` (or `session/load`) reply, holding `configOptions`/`modes`/`models`.
    setup: Value,
    /// Slash commands from `available_commands_update`.
    commands: Vec<Value>,
    /// The agent's task list, from `plan` updates.
    todos: Vec<Value>,
}

/// One ACP agent, serving opman's opencode-compatible contract.
pub struct AcpEngine {
    /// Config key for this agent (`claude`, …). Also its provider id.
    pub id: String,
    pub agent: AgentConfig,
    sessions: Mutex<HashMap<String, Session>>,
    /// Rendered conversations, folded from the update stream.
    transcripts: Mutex<HashMap<String, Transcript>>,
    discovered: Mutex<HashMap<String, Discovered>>,
    /// opman session id → agent session id, and the reverse for routing inbound updates.
    acp_ids: Mutex<HashMap<String, String>>,
    /// Sessions currently replaying a `session/load`.
    replaying: Mutex<HashMap<String, bool>>,
    /// Sessions whose history has already been asked for, so a cold read connects once
    /// rather than respawning the agent on every poll of an empty conversation.
    hydrated: Mutex<HashSet<String>>,
    /// Whether the agent advertised `loadSession` in its `initialize` reply. `None` until
    /// the first handshake answers the question.
    load_capable: Mutex<Option<bool>>,
    /// Sessions with a `session/prompt` still outstanding.
    inflight: Mutex<HashMap<String, bool>>,
    /// A follow-up typed during a turn, for agents that cannot be steered mid-turn. Held
    /// whole, attachments included: a queued prompt must arrive as the user composed it.
    followups: Mutex<HashMap<String, attach::Prompt>>,
    /// The agent's `session/new` reply from the startup probe, so models and modes are
    /// known before any user session exists.
    capabilities: Mutex<Value>,
    conns: ConnMap,
    events: broadcast::Sender<EngineEvent>,
    /// Raw event channel for in-process consumers, bypassing HTTP SSE buffering (which
    /// batches frames and would undo the point of per-token streaming).
    raw_events: broadcast::Sender<String>,
    pending:
        Mutex<HashMap<String, tokio::sync::oneshot::Sender<crate::claude_engine::PendingReply>>>,
    persist: Option<PathBuf>,
    url: Mutex<String>,
    exe: PathBuf,
    mcp_flags: (bool, bool, bool, bool),
}

impl AcpEngine {
    fn new(
        id: String,
        agent: AgentConfig,
        persist: Option<PathBuf>,
        mcp_flags: (bool, bool, bool, bool),
    ) -> Self {
        let (events, _) = broadcast::channel(2048);
        let (raw_events, _) = broadcast::channel(2048);
        Self {
            id,
            agent,
            sessions: Mutex::new(session::load_sessions(&persist)),
            transcripts: Mutex::new(HashMap::new()),
            discovered: Mutex::new(HashMap::new()),
            acp_ids: Mutex::new(HashMap::new()),
            replaying: Mutex::new(HashMap::new()),
            hydrated: Mutex::new(HashSet::new()),
            load_capable: Mutex::new(None),
            inflight: Mutex::new(HashMap::new()),
            followups: Mutex::new(HashMap::new()),
            capabilities: Mutex::new(Value::Null),
            conns: ConnMap::default(),
            events,
            raw_events,
            pending: Mutex::new(HashMap::new()),
            persist,
            url: Mutex::new(String::new()),
            exe: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("opman")),
            mcp_flags,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    /// Subscribe to raw event payloads, for in-process consumers that must not be batched.
    pub fn subscribe_raw(&self) -> broadcast::Receiver<String> {
        self.raw_events.subscribe()
    }

    /// Emit an opencode-shaped `{type, properties}` event scoped to `directory`.
    pub fn emit(&self, directory: &str, event_type: &str, properties: Value) {
        let data = json!({ "type": event_type, "properties": properties }).to_string();
        let _ = self.events.send(EngineEvent {
            directory: directory.to_string(),
            data: data.clone(),
        });
        let _ = self.raw_events.send(data);
    }
}

/// Turn ACP's per-turn usage report into opman's token shape. The `claude -p` engine had no
/// equivalent channel, which is why its sessions always showed zero.
fn usage_tokens(usage: &Value) -> Value {
    let field = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    json!({
        "input": field("inputTokens"),
        "output": field("outputTokens"),
        "reasoning": field("reasoningTokens"),
        "cache": { "read": field("cachedReadTokens"), "write": field("cachedWriteTokens") },
    })
}

/// Emit a one-off system bubble for signals that never reach the transcript: a spawn
/// failure, a dropped connection, a truncated turn.
fn emit_system(engine: &Arc<AcpEngine>, session_id: &str, level: &str, text: &str) {
    let Some(session) = engine.get_session(session_id) else {
        return;
    };
    let variant = match level {
        "error" => "error",
        "warning" | "warn" => "warning",
        _ => "notification",
    };
    let ts = now_ms();
    let mid = format!("msg_sys_{session_id}_{ts}");
    engine.emit(
        &session.directory,
        "message.updated",
        json!({ "info": {
            "role": "system", "variant": variant, "level": level,
            "id": mid, "sessionID": session_id,
            "time": { "created": ts, "completed": ts },
        }}),
    );
    engine.emit(
        &session.directory,
        "message.part.updated",
        json!({ "sessionID": session_id, "time": ts, "part": {
            "type": "text", "id": format!("{mid}:0"),
            "messageID": mid, "sessionID": session_id, "text": text,
        }}),
    );
}

/// Start an embedded server for one configured agent. Returns `(base_url, handle, engine)`
/// so the caller can subscribe to the raw event channel directly.
pub async fn start_embedded_server(
    id: &str,
    agent: AgentConfig,
    mcp_flags: (bool, bool, bool, bool),
) -> Result<(String, ServerHandle, Arc<AcpEngine>)> {
    let persist =
        dirs::config_dir().map(|d| d.join("opman").join(format!("acp_{id}_sessions.json")));
    let engine = Arc::new(AcpEngine::new(id.to_string(), agent, persist, mcp_flags));

    // Discover the agent's models and modes off the startup path, so `/provider` serves a
    // real catalogue rather than an empty list the picker cannot repair.
    {
        let probing = engine.clone();
        let label = id.to_string();
        tokio::spawn(async move {
            match conn::probe_capabilities(&probing).await {
                Ok(setup) => probing.set_capabilities(setup),
                Err(e) => tracing::warn!(agent = %label, "ACP capability probe failed: {e}"),
            }
        });
    }

    let app = routes::router(engine.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .with_context(|| format!("failed to bind embedded ACP engine port for `{id}`"))?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}");
    engine.set_url(&url);
    info!(agent = %id, %url, "ACP engine ready");

    let label = id.to_string();
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(agent = %label, "ACP engine server error: {e}");
        }
    });

    let handle: ServerHandle = Arc::new(std::sync::Mutex::new(None));
    Ok((url, handle, engine))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
