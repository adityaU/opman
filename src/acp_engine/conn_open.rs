//! Starting and opening ACP connections: spawn + handshake, `session/new` (with the
//! login retry), `session/load`, and the startup capability probe.

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::{json, Value};
use tracing::debug;

use super::super::client::Client;
use super::super::failure::{startup_failure, StderrTail};
use super::super::jsonrpc::{self, Peer};
use super::super::launch;
use super::super::AcpEngine;
use super::{Pooled, NEXT_INSTANCE_ID};

/// Ask the agent what it offers, before any user session exists.
///
/// Models, modes and effort levels are only knowable from a `session/new` reply, so without
/// this the engine picker is empty until the user's first message — the catalogue would
/// arrive after the moment it is needed. One throwaway session at startup fills it in. The
/// scratch cwd keeps the probe cheap: the agent has no project to load.
pub(in super::super) async fn probe_capabilities(engine: &Arc<AcpEngine>) -> Result<Value> {
    let dir = std::env::temp_dir().join(format!("opman-acp-probe-{}", engine.id));
    std::fs::create_dir_all(&dir)?;
    let dir = dir.to_string_lossy().to_string();

    super::super::archive::ensure_installed(&engine.agent).await?;
    let mut child = launch::spawn(&engine.agent, &dir)?;
    let tail = StderrTail::capture(&mut child);
    let stdin = child.stdin.take().context("acp probe child has no stdin")?;
    let stdout = child
        .stdout
        .take()
        .context("acp probe child has no stdout")?;
    let peer = Peer::new(stdin, stdout, Client::new(engine.clone()));
    match probe_session(engine, &peer, &dir).await {
        Ok(setup) => {
            // `kill_on_drop` handles the child, but waiting keeps it from lingering as a
            // zombie until the engine is dropped.
            let _ = child.start_kill();
            let _ = child.wait().await;
            Ok(setup)
        }
        Err(error) => Err(startup_failure(&mut child, &tail, error).await),
    }
}

async fn probe_session(engine: &Arc<AcpEngine>, peer: &Peer, dir: &str) -> Result<Value> {
    let negotiated = handshake_in_time(engine, peer).await?;
    // Whether old sessions can be reopened is answered here, before any of them is opened:
    // a history read must not have to spawn a child just to discover the agent cannot help.
    engine.note_load_capable(negotiated.loads);
    // No MCP servers: the probe never runs a turn, and starting opman's own servers for a
    // session that is about to be discarded is pure cost.
    let meta = engine.agent.session_meta();
    let (_, setup) = open_session(peer, &negotiated.init, dir, &json!([]), meta.as_ref()).await?;
    Ok(setup)
}

/// How long an agent gets to answer `initialize`. Generous: a first start may download.
const HANDSHAKE_LIMIT: std::time::Duration = std::time::Duration::from_secs(120);

async fn handshake_in_time(
    engine: &Arc<AcpEngine>,
    peer: &Peer,
) -> Result<super::super::handshake::Negotiated> {
    let handshake = super::super::handshake::negotiate(engine, peer);
    match tokio::time::timeout(HANDSHAKE_LIMIT, handshake).await {
        Ok(outcome) => outcome,
        Err(_) => anyhow::bail!(
            "the agent did not answer `initialize` within {} seconds",
            HANDSHAKE_LIMIT.as_secs()
        ),
    }
}

/// Spawn the configured server and negotiate the process-wide capabilities.
pub(super) async fn spawn_and_negotiate(engine: &Arc<AcpEngine>, dir: &str) -> Result<Pooled> {
    // An agent that ships as an archive is downloaded on its first start.
    super::super::archive::ensure_installed(&engine.agent).await?;
    let mut child = launch::spawn(&engine.agent, dir)?;
    let stderr = StderrTail::capture(&mut child);
    let stdin = child.stdin.take().context("acp child has no stdin")?;
    let stdout = child.stdout.take().context("acp child has no stdout")?;
    let peer = Peer::new(stdin, stdout, Client::new(engine.clone()));

    // Which remote MCP transports this agent can dial for itself must be known before any
    // session is created: it decides whether a remote server reaches the agent directly or
    // through opman's local proxy.
    let negotiated = match handshake_in_time(engine, &peer).await {
        Ok(negotiated) => negotiated,
        Err(error) => return Err(startup_failure(&mut child, &stderr, error).await),
    };
    engine.note_load_capable(negotiated.loads);
    Ok(Pooled {
        peer,
        child,
        stderr,
        instance: NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed),
        negotiated,
        attached: HashSet::new(),
    })
}

/// Open a session, logging in first if that is what the agent is holding out for.
///
/// ACP puts authentication behind a specific rejection rather than a capability flag: the
/// agent answers `session/new` with `auth_required` and expects the client to call
/// `authenticate` and try again. Retrying exactly once is the whole protocol — a second
/// refusal after a successful login is a real failure, not a loop to keep running.
pub(super) async fn open_session(
    peer: &Peer,
    init: &Value,
    dir: &str,
    mcp: &Value,
    meta: Option<&Value>,
) -> Result<(String, Value)> {
    match new_session(peer, dir, mcp, meta).await {
        Err(refused) if jsonrpc::needs_auth(&refused) => {
            let method = super::super::handshake::authenticate(peer, init).await?;
            debug!(%method, "acp: authenticated, retrying session/new");
            new_session(peer, dir, mcp, meta)
                .await
                .context("ACP `session/new` failed after authenticating")
        }
        outcome => outcome.context("ACP `session/new` failed"),
    }
}

/// `session/new` or `session/load` params. `meta` carries the agent's native-tool switches.
fn session_params(dir: &str, mcp: &Value, meta: Option<&Value>) -> Value {
    let mut params = json!({ "cwd": dir, "mcpServers": mcp });
    if let (Some(meta), Some(object)) = (meta, params.as_object_mut()) {
        object.insert("_meta".to_string(), meta.clone());
    }
    params
}

async fn new_session(
    peer: &Peer,
    dir: &str,
    mcp: &Value,
    meta: Option<&Value>,
) -> Result<(String, Value)> {
    let result = peer
        .request("session/new", session_params(dir, mcp, meta))
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
pub(super) async fn load_session(
    engine: &Arc<AcpEngine>,
    peer: &Peer,
    session_id: &str,
    dir: &str,
    prior: &str,
    negotiated: &super::super::handshake::Negotiated,
    servers: &Value,
    meta: Option<&Value>,
) -> Result<(String, Value)> {
    engine.bind_acp_session(session_id, prior);
    engine.begin_replay(session_id);
    let mut params = session_params(dir, servers, meta);
    if let Some(object) = params.as_object_mut() {
        object.insert("sessionId".to_string(), json!(prior));
    }
    let outcome = peer.request("session/load", params).await;
    let emits = engine.end_replay(session_id);
    super::super::render::broadcast(engine, session_id, emits);
    match outcome {
        Ok(result) => Ok((prior.to_string(), result)),
        Err(e) => {
            // A stale id must not wedge the session: forget it and start a fresh
            // conversation instead of failing every future prompt.
            debug!(session = %session_id, "acp session/load failed, starting fresh: {e}");
            engine.forget_acp_session(session_id);
            open_session(peer, &negotiated.init, dir, servers, meta).await
        }
    }
}

#[cfg(test)]
#[path = "conn_tests.rs"]
mod conn_tests;
