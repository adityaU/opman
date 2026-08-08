//! Connection lifecycle: spawn an ACP server, negotiate, and drive turns.
//!
//! One child per opman session, as with the `claude -p` engine, but the semantics are the
//! protocol's rather than a pipe's:
//! - a follow-up mid-turn is another `session/prompt`, which agents that advertise
//!   steering deliver to the running model (true steering, not a queue);
//! - abort is `session/cancel`, so the agent unwinds and reports `stopReason: cancelled`
//!   instead of being killed and losing the turn;
//! - continuity after a restart is `session/load`, which replays history over
//!   `session/update` rather than re-reading a transcript file.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::debug;

use super::attach::PromptCaps;
use super::client::Client;
use super::jsonrpc::{self, Peer};
use super::AcpEngine;

/// A live ACP server bound to one opman session.
pub struct Conn {
    peer: Peer,
    /// The agent's own session id, used in every `session/*` call.
    acp_session: String,
    child: Child,
    /// Whether the agent accepts a prompt while one is already running.
    steering: bool,
    /// Which non-text content blocks the agent will accept in a prompt.
    prompt_caps: PromptCaps,
}

/// What a turn needs from an established connection.
pub(super) struct Ready {
    pub peer: Peer,
    pub acp_session: String,
    pub steering: bool,
    pub prompt_caps: PromptCaps,
}

/// Live connections keyed by opman session id.
#[derive(Default)]
pub struct ConnMap(Mutex<HashMap<String, Conn>>);

impl ConnMap {
    /// Drop a session's connection, killing the child. Idempotent.
    pub async fn close(&self, session_id: &str) {
        let removed = self.0.lock().await.remove(session_id);
        if let Some(mut conn) = removed {
            let _ = conn.child.start_kill();
            let _ = conn.child.wait().await;
        }
    }

    /// Drop every connection, killing the children.
    ///
    /// Used when an agent is reconfigured or removed from `acp.json`: those processes were
    /// launched from the definition that no longer exists, so leaving them running would
    /// mean a session still talking to config the user just deleted.
    pub async fn close_all(&self) {
        let drained: Vec<Conn> = self.0.lock().await.drain().map(|(_, conn)| conn).collect();
        for mut conn in drained {
            let _ = conn.child.start_kill();
            let _ = conn.child.wait().await;
        }
    }

    /// The session's connection, establishing one on first use. Returns what a turn needs —
    /// never the `Conn` itself, so the child handle stays owned by the map and cannot be
    /// dropped (and so killed) by a caller holding it across an await.
    pub(super) async fn ensure(&self, engine: &Arc<AcpEngine>, session_id: &str) -> Result<Ready> {
        let mut conns = self.0.lock().await;
        if !conns.contains_key(session_id) {
            let conn = establish(engine, session_id).await?;
            conns.insert(session_id.to_string(), conn);
        }
        let conn = conns
            .get(session_id)
            .context("connection vanished during setup")?;
        Ok(Ready {
            peer: conn.peer.clone(),
            acp_session: conn.acp_session.clone(),
            steering: conn.steering,
            prompt_caps: conn.prompt_caps,
        })
    }

    /// An already-established connection, or None. Used by abort, which must not start one.
    pub(super) async fn existing(&self, session_id: &str) -> Option<(Peer, String)> {
        let conns = self.0.lock().await;
        conns
            .get(session_id)
            .map(|c| (c.peer.clone(), c.acp_session.clone()))
    }
}

/// Ask the agent what it offers, before any user session exists.
///
/// Models, modes and effort levels are only knowable from a `session/new` reply, so without
/// this the engine picker is empty until the user's first message — the catalogue would
/// arrive after the moment it is needed. One throwaway session at startup fills it in. The
/// scratch cwd keeps the probe cheap: the agent has no project to load.
pub(super) async fn probe_capabilities(engine: &Arc<AcpEngine>) -> Result<Value> {
    let dir = std::env::temp_dir().join(format!("opman-acp-probe-{}", engine.id));
    std::fs::create_dir_all(&dir)?;
    let dir = dir.to_string_lossy().to_string();

    let mut child = spawn(engine, &dir, "probe")?;
    let stdin = child.stdin.take().context("acp probe child has no stdin")?;
    let stdout = child
        .stdout
        .take()
        .context("acp probe child has no stdout")?;
    let peer = Peer::new(stdin, stdout, Client::new(engine.clone()));
    let negotiated = super::handshake::negotiate(engine, &peer)
        .await
        .context("ACP handshake failed during capability probe")?;
    // Whether old sessions can be reopened is answered here, before any of them is opened:
    // a history read must not have to spawn a child just to discover the agent cannot help.
    engine.note_load_capable(negotiated.loads);
    // No MCP servers: the probe never runs a turn, and starting opman's own servers for a
    // session that is about to be discarded is pure cost.
    let (_, setup) = open_session(&peer, &negotiated.init, &dir, &json!([])).await?;
    // `kill_on_drop` handles the child, but waiting keeps it from lingering as a zombie
    // until the engine is dropped.
    let _ = child.start_kill();
    let _ = child.wait().await;
    Ok(setup)
}

/// Spawn the configured server and bring up a session on it.
async fn establish(engine: &Arc<AcpEngine>, session_id: &str) -> Result<Conn> {
    let session = engine
        .get_session(session_id)
        .context("unknown opman session")?;
    let dir = session.directory;
    if dir.is_empty() {
        bail!("session has no working directory");
    }
    let mut child = spawn(engine, &dir, session_id)?;
    let stdin = child.stdin.take().context("acp child has no stdin")?;
    let stdout = child.stdout.take().context("acp child has no stdout")?;
    let peer = Peer::new(stdin, stdout, Client::new(engine.clone()));

    // Which remote MCP transports this agent can dial for itself must be known before any
    // session is created: it decides whether a remote server reaches the agent directly or
    // through opman's local proxy.
    let negotiated = super::handshake::negotiate(engine, &peer).await?;
    let (steering, prompt_caps) = (negotiated.steering, negotiated.prompt_caps);
    engine.note_load_capable(negotiated.loads);

    // Resume the prior conversation when the agent supports it; otherwise start clean.
    let resume = session
        .acp_session
        .filter(|_| negotiated.loads)
        .filter(|id| !id.is_empty());
    let servers = engine.mcp_servers(&dir, session_id, negotiated.mcp_caps);
    let (acp_session, setup) = match resume {
        Some(prior) => {
            load_session(
                engine,
                &peer,
                session_id,
                &dir,
                &prior,
                &negotiated,
                &servers,
            )
            .await?
        }
        None => open_session(&peer, &negotiated.init, &dir, &servers).await?,
    };

    engine.bind_acp_session(session_id, &acp_session);
    engine.merge_session_setup(session_id, &setup);
    super::conn_options::apply_defaults(engine, &peer, session_id, &acp_session, &setup).await;
    Ok(Conn {
        peer,
        acp_session,
        child,
        steering,
        prompt_caps,
    })
}

/// Open a session, logging in first if that is what the agent is holding out for.
///
/// ACP puts authentication behind a specific rejection rather than a capability flag: the
/// agent answers `session/new` with `auth_required` and expects the client to call
/// `authenticate` and try again. Retrying exactly once is the whole protocol — a second
/// refusal after a successful login is a real failure, not a loop to keep running.
async fn open_session(
    peer: &Peer,
    init: &Value,
    dir: &str,
    mcp: &Value,
) -> Result<(String, Value)> {
    match new_session(peer, dir, mcp).await {
        Err(refused) if jsonrpc::needs_auth(&refused) => {
            let method = super::handshake::authenticate(peer, init).await?;
            debug!(%method, "acp: authenticated, retrying session/new");
            new_session(peer, dir, mcp)
                .await
                .context("ACP `session/new` failed after authenticating")
        }
        outcome => outcome.context("ACP `session/new` failed"),
    }
}

async fn new_session(peer: &Peer, dir: &str, mcp: &Value) -> Result<(String, Value)> {
    let result = peer
        .request("session/new", json!({ "cwd": dir, "mcpServers": mcp }))
        .await?;
    let id = result
        .get("sessionId")
        .and_then(Value::as_str)
        .context("`session/new` returned no sessionId")?
        .to_string();
    Ok((id, result))
}

/// Replay a prior conversation. The agent re-sends its history as `session/update`
/// notifications, so the transcript is cleared first and rebuilt from the replay.
async fn load_session(
    engine: &Arc<AcpEngine>,
    peer: &Peer,
    session_id: &str,
    dir: &str,
    prior: &str,
    negotiated: &super::handshake::Negotiated,
    servers: &Value,
) -> Result<(String, Value)> {
    engine.bind_acp_session(session_id, prior);
    engine.begin_replay(session_id);
    let params = json!({
        "sessionId": prior,
        "cwd": dir,
        "mcpServers": servers,
    });
    let outcome = peer.request("session/load", params).await;
    let emits = engine.end_replay(session_id);
    super::render::broadcast(engine, session_id, emits);
    match outcome {
        Ok(result) => Ok((prior.to_string(), result)),
        Err(e) => {
            // A stale id must not wedge the session: forget it and start a fresh
            // conversation instead of failing every future prompt.
            debug!(session = %session_id, "acp session/load failed, starting fresh: {e}");
            engine.forget_acp_session(session_id);
            open_session(peer, &negotiated.init, dir, servers).await
        }
    }
}

fn spawn(engine: &Arc<AcpEngine>, dir: &str, session_id: &str) -> Result<Child> {
    let agent = &engine.agent;
    let mut cmd = Command::new(&agent.command);
    cmd.args(&agent.args);
    for key in agent.env_removals() {
        cmd.env_remove(key);
    }
    for (key, value) in &agent.env {
        cmd.env(key, value);
    }
    cmd.env("OPENCODE_SESSION_ID", session_id);
    cmd.current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        // The agent's own logs are its business; keeping them off opman's stdout avoids
        // interleaving them with the TUI.
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    cmd.spawn().with_context(|| {
        format!(
            "failed to spawn ACP agent `{}` (is it installed?)",
            agent.command
        )
    })
}

#[cfg(test)]
#[path = "conn_tests.rs"]
mod conn_tests;
