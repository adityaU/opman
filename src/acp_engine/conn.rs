//! Connection lifecycle: spawn an ACP server, negotiate, and drive turns.
//!
//! One pooled child per agent and working directory, but the semantics are the protocol's
//! rather than a pipe's:
//! - a follow-up mid-turn is another `session/prompt`, which agents that advertise
//!   steering deliver to the running model (true steering, not a queue);
//! - abort is `session/cancel`, so the agent unwinds and reports `stopReason: cancelled`
//!   instead of being killed and losing the turn;
//! - continuity after a restart is `session/load`, which replays history over
//!   `session/update` rather than re-reading a transcript file.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, OnceCell};
use tracing::debug;

use super::attach::PromptCaps;
use super::client::Client;
use super::jsonrpc::{self, Peer};
use super::AcpEngine;

type PoolKey = String;

static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

/// A live ACP server shared by one or more opman sessions.
struct Pooled {
    peer: Peer,
    child: Child,
    /// Monotonic identity of this live child, distinct from its pool key.
    instance: u64,
    /// Capabilities settled by the process handshake, reused by every session it serves.
    negotiated: super::handshake::Negotiated,
    /// Opman sessions currently bound to this child.
    attached: HashSet<String>,
}

#[derive(Clone)]
struct Bound {
    key: PoolKey,
    /// The live child this binding belongs to.
    instance: u64,
    /// The agent's own session id, used in every `session/*` call.
    acp_session: String,
}

/// What a turn needs from an established connection.
pub(super) struct Ready {
    pub peer: Peer,
    pub acp_session: String,
    pub steering: bool,
    pub prompt_caps: PromptCaps,
}

/// Live children keyed by directory (or by session id when sharing is disabled), plus the
/// per-session binding that tells ACP traffic which session it belongs to.
#[derive(Default)]
pub struct ConnMap {
    /// The outer lock is held only while looking up or inserting a slot. The `OnceCell`
    /// makes concurrent first users await one spawn and handshake rather than racing them.
    pool: Mutex<HashMap<PoolKey, Arc<OnceCell<Arc<Mutex<Pooled>>>>>>,
    bound: Mutex<HashMap<String, Bound>>,
}

impl ConnMap {
    /// Detach a session from its child. The child is killed only when its last session leaves.
    pub async fn close(&self, session_id: &str) {
        let Some(bound) = self.bound.lock().await.remove(session_id) else {
            return;
        };
        let slot = self.pool.lock().await.get(&bound.key).cloned();
        let Some(slot) = slot else {
            return;
        };
        let Some(entry) = slot.get().cloned() else {
            return;
        };
        let mut pooled = entry.lock().await;
        if pooled.instance != bound.instance {
            return;
        }
        pooled.attached.remove(session_id);
        drop(pooled);
        if let Some(entry) = self.remove_entry(&bound.key, &slot).await {
            kill_entry(entry).await;
        }
    }

    /// The child is gone or wedged. Drop it and unbind every session on it, so each one
    /// reconnects (and `session/load`s) on its next prompt. Returns the other affected
    /// sessions so callers can tell their panes what happened.
    pub(super) async fn evict(&self, session_id: &str) -> Vec<String> {
        let Some(target) = self.bound.lock().await.get(session_id).cloned() else {
            return Vec::new();
        };
        let entry = {
            let mut pool = self.pool.lock().await;
            let Some(slot) = pool.get(&target.key).cloned() else {
                return Vec::new();
            };
            let Some(entry) = slot.get().cloned() else {
                return Vec::new();
            };
            if entry.lock().await.instance != target.instance {
                return Vec::new();
            }
            pool.remove(&target.key);
            entry
        };
        let attached: Vec<String> = entry.lock().await.attached.drain().collect();
        let mut bound = self.bound.lock().await;
        let mut affected = Vec::new();
        for id in &attached {
            if bound
                .get(id)
                .is_some_and(|current| current.instance == target.instance)
            {
                bound.remove(id);
                if id != session_id {
                    affected.push(id.clone());
                }
            }
        }
        drop(bound);
        kill_entry(entry).await;
        affected
    }

    /// Drop every connection, killing the children.
    ///
    /// Used when an agent is reconfigured or removed from `acp.json`: those processes were
    /// launched from the definition that no longer exists, so leaving them running would
    /// mean a session still talking to config the user just deleted.
    pub async fn close_all(&self) {
        let slots: Vec<_> = self
            .pool
            .lock()
            .await
            .drain()
            .map(|(_, slot)| slot)
            .collect();
        self.bound.lock().await.clear();
        for slot in slots {
            if let Some(entry) = slot.get() {
                kill_entry(entry.clone()).await;
            }
        }
    }

    /// The session's connection, establishing one on first use. Returns what a turn needs —
    /// never the pooled entry itself, so the child handle stays owned by the map and cannot be
    /// dropped (and so killed) by a caller holding it across an await.
    pub(super) async fn ensure(&self, engine: &Arc<AcpEngine>, session_id: &str) -> Result<Ready> {
        let session = engine
            .get_session(session_id)
            .context("unknown opman session")?;
        let dir = session.directory;
        if dir.is_empty() {
            bail!("session has no working directory");
        }
        let key = pool_key(engine, session_id, &dir);
        let slot = {
            let mut pool = self.pool.lock().await;
            pool.entry(key.clone())
                .or_insert_with(|| Arc::new(OnceCell::new()))
                .clone()
        };
        let engine_for_spawn = engine.clone();
        let dir_for_spawn = dir.clone();
        let entry = slot
            .get_or_try_init(|| async move {
                let pooled = spawn_and_negotiate(&engine_for_spawn, &dir_for_spawn).await?;
                Ok::<_, anyhow::Error>(Arc::new(Mutex::new(pooled)))
            })
            .await?
            .clone();

        let mut pooled = entry.lock().await;
        let current = self.bound.lock().await.get(session_id).cloned();
        if let Some(current) = current {
            if current.instance == pooled.instance {
                return Ok(ready(&pooled, &current.acp_session));
            }
        }

        let previous = { self.bound.lock().await.get(session_id).cloned() };
        if let Some(previous) = previous {
            self.detach_previous(session_id, &previous, &key).await;
        }

        let servers = engine.mcp_servers(&dir, session_id, pooled.negotiated.mcp_caps);
        let resume = session
            .acp_session
            .filter(|_| pooled.negotiated.loads)
            .filter(|id| !id.is_empty());
        let (acp_session, setup) = match resume {
            Some(prior) => {
                load_session(
                    engine,
                    &pooled.peer,
                    session_id,
                    &dir,
                    &prior,
                    &pooled.negotiated,
                    &servers,
                )
                .await?
            }
            None => open_session(&pooled.peer, &pooled.negotiated.init, &dir, &servers).await?,
        };
        engine.bind_acp_session(session_id, &acp_session);
        engine.merge_session_setup(session_id, &setup);
        super::conn_options::apply_defaults(engine, &pooled.peer, session_id, &acp_session, &setup)
            .await;
        pooled.attached.insert(session_id.to_string());
        self.bound.lock().await.insert(
            session_id.to_string(),
            Bound {
                key,
                instance: pooled.instance,
                acp_session: acp_session.clone(),
            },
        );
        Ok(ready(&pooled, &acp_session))
    }

    /// An already-established connection, or None. Used by abort, which must not start one.
    pub(super) async fn existing(&self, session_id: &str) -> Option<(Peer, String)> {
        let (key, instance, acp_session) = {
            let bound = self.bound.lock().await;
            let bound = bound.get(session_id)?;
            (bound.key.clone(), bound.instance, bound.acp_session.clone())
        };
        let slot = self.pool.lock().await.get(&key).cloned()?;
        let entry = slot.get().cloned()?;
        let pooled = entry.lock().await;
        if pooled.instance != instance {
            return None;
        }
        Some((pooled.peer.clone(), acp_session))
    }

    async fn remove_entry(
        &self,
        key: &str,
        expected: &Arc<OnceCell<Arc<Mutex<Pooled>>>>,
    ) -> Option<Arc<Mutex<Pooled>>> {
        let mut pool = self.pool.lock().await;
        if !pool
            .get(key)
            .is_some_and(|entry| Arc::ptr_eq(entry, expected))
        {
            return None;
        }
        let entry = expected.get()?.clone();
        if !entry.lock().await.attached.is_empty() {
            return None;
        }
        pool.remove(key);
        Some(entry)
    }

    async fn detach_previous(&self, session_id: &str, previous: &Bound, current_key: &str) {
        let removed = {
            let mut bound = self.bound.lock().await;
            if bound
                .get(session_id)
                .is_some_and(|current| current.instance == previous.instance)
            {
                bound.remove(session_id);
                true
            } else {
                false
            }
        };
        if !removed {
            return;
        }
        // The old child was already removed from this key's slot. There is no old
        // entry left to detach when a new child has taken the same key.
        if previous.key == current_key {
            return;
        }
        let Some(slot) = self.pool.lock().await.get(&previous.key).cloned() else {
            return;
        };
        let Some(entry) = slot.get().cloned() else {
            return;
        };
        {
            let mut pooled = entry.lock().await;
            if pooled.instance != previous.instance {
                return;
            }
            pooled.attached.remove(session_id);
        }
        if let Some(entry) = self.remove_entry(&previous.key, &slot).await {
            kill_entry(entry).await;
        }
    }
}

fn pool_key(engine: &Arc<AcpEngine>, session_id: &str, dir: &str) -> PoolKey {
    if engine.agent.shared_process {
        dir.to_string()
    } else {
        session_id.to_string()
    }
}

fn ready(pooled: &Pooled, acp_session: &str) -> Ready {
    Ready {
        peer: pooled.peer.clone(),
        acp_session: acp_session.to_string(),
        steering: pooled.negotiated.steering,
        prompt_caps: pooled.negotiated.prompt_caps,
    }
}

async fn kill_entry(entry: Arc<Mutex<Pooled>>) {
    let mut pooled = entry.lock().await;
    let _ = pooled.child.start_kill();
    let _ = pooled.child.wait().await;
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

    let mut child = spawn(engine, &dir)?;
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

/// Spawn the configured server and negotiate the process-wide capabilities.
async fn spawn_and_negotiate(engine: &Arc<AcpEngine>, dir: &str) -> Result<Pooled> {
    let mut child = spawn(engine, dir)?;
    let stdin = child.stdin.take().context("acp child has no stdin")?;
    let stdout = child.stdout.take().context("acp child has no stdout")?;
    let peer = Peer::new(stdin, stdout, Client::new(engine.clone()));

    // Which remote MCP transports this agent can dial for itself must be known before any
    // session is created: it decides whether a remote server reaches the agent directly or
    // through opman's local proxy.
    let negotiated = match super::handshake::negotiate(engine, &peer).await {
        Ok(negotiated) => negotiated,
        Err(error) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(error);
        }
    };
    engine.note_load_capable(negotiated.loads);
    Ok(Pooled {
        peer,
        child,
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

fn spawn(engine: &Arc<AcpEngine>, dir: &str) -> Result<Child> {
    let agent = &engine.agent;
    let mut cmd = Command::new(&agent.command);
    cmd.args(&agent.args);
    for key in agent.env_removals() {
        cmd.env_remove(key);
    }
    for (key, value) in &agent.env {
        cmd.env(key, value);
    }
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

#[cfg(all(test, unix))]
#[path = "conn_pool_tests.rs"]
mod conn_pool_tests;
