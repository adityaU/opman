//! A one-shot health probe of an ACP agent, for `opman doctor`.
//!
//! It runs the same start-up path the engine runs (same launch rules, same `initialize`,
//! same `session/new`) but keeps every answer instead of using it. Nothing is pooled and no
//! login is attempted: a login is a side effect, and the doctor only needs to know that one
//! is required.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::client::Client;
use super::config::AgentConfig;
use super::failure::{startup_failure, StderrTail};
use super::jsonrpc::{self, Peer};
use super::options::{self, Choice};
use super::{launch, AcpEngine};

/// What the agent said about itself in its `initialize` reply.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Hello {
    pub protocol: u64,
    pub name: Option<String>,
    pub version: Option<String>,
    pub auth_methods: Vec<String>,
    pub loads_sessions: bool,
}

/// What the agent offers in a fresh session. Each list holds display names.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Offer {
    pub models: Vec<String>,
    pub current_model: Option<String>,
    /// Permission modes, or the agent's own "agents" when `modes_are_agents` is set.
    pub modes: Vec<String>,
    pub efforts: Vec<String>,
}

/// How `session/new` ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Session {
    Offered(Offer),
    /// The agent wants a login first. The doctor does not log in.
    LoginRequired,
    /// The agent refused, with its exact reason.
    Failed(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Probe {
    pub hello: Hello,
    pub session: Session,
}

/// Start `agent`, ask it what it offers, and stop it. `Err` is the exact reason it could not
/// even finish `initialize` (spawn error, exit status and stderr, JSON-RPC error, timeout).
pub(crate) async fn probe(id: &str, agent: &AgentConfig, limit: Duration) -> Result<Probe, String> {
    let dir = std::env::temp_dir().join(format!("opman-doctor-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let outcome = run(id, agent, &dir.to_string_lossy(), limit).await;
    let _ = std::fs::remove_dir(&dir);
    outcome
}

async fn run(id: &str, agent: &AgentConfig, dir: &str, limit: Duration) -> Result<Probe, String> {
    let mut child = launch::spawn(agent, dir).map_err(|e| format!("{e:#}"))?;
    let tail = StderrTail::capture(&mut child);
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill().await;
        return Err("the agent process has no stdin/stdout pipes".to_string());
    };
    let engine = engine(id, agent);
    let peer = Peer::new(stdin, stdout, Client::new(engine.clone()));
    let result = exchange(&engine, &peer, dir, limit).await;
    match result {
        Err(error) => Err(format!(
            "{:#}",
            startup_failure(&mut child, &tail, error).await
        )),
        Ok(mut probe) => {
            let _ = child.kill().await;
            if let Session::Failed(reason) = &mut probe.session {
                let stderr = tail.text().await;
                if !stderr.is_empty() {
                    reason.push_str(&format!("\nAgent stderr (last part):\n{stderr}"));
                }
            }
            Ok(probe)
        }
    }
}

fn engine(id: &str, agent: &AgentConfig) -> Arc<AcpEngine> {
    let flags = crate::mcp_registry::BuiltinFlags::default();
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    Arc::new(AcpEngine::new(
        id.to_string(),
        agent.clone(),
        None,
        registry,
    ))
}

/// The two requests, each with its own time limit. Split from [`run`] so a test can drive it
/// over an in-memory peer.
pub(super) async fn exchange(
    engine: &Arc<AcpEngine>,
    peer: &Peer,
    dir: &str,
    limit: Duration,
) -> Result<Probe> {
    let negotiated = within(
        limit,
        "initialize",
        super::handshake::negotiate(engine, peer),
    )
    .await?;
    let hello = hello(&negotiated.init, negotiated.loads);
    let request = peer.request("session/new", json!({ "cwd": dir, "mcpServers": [] }));
    let session = match within(limit, "session/new", request).await {
        Ok(reply) => Session::Offered(offer(&reply)),
        Err(error) if jsonrpc::needs_auth(&error) => Session::LoginRequired,
        Err(error) => Session::Failed(format!("{error:#}")),
    };
    Ok(Probe { hello, session })
}

async fn within<T>(
    limit: Duration,
    step: &str,
    work: impl Future<Output = Result<T>>,
) -> Result<T> {
    match tokio::time::timeout(limit, work).await {
        Ok(outcome) => outcome,
        Err(_) => bail!(
            "the agent did not answer `{step}` within {} seconds",
            limit.as_secs()
        ),
    }
}

fn hello(init: &Value, loads_sessions: bool) -> Hello {
    let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_string);
    let info = init.get("agentInfo");
    Hello {
        protocol: init
            .get("protocolVersion")
            .and_then(Value::as_u64)
            .unwrap_or(1),
        name: text(
            info.and_then(|i| i.get("title"))
                .or_else(|| info.and_then(|i| i.get("name"))),
        ),
        version: text(info.and_then(|i| i.get("version"))),
        auth_methods: init
            .get("authMethods")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| text(m.get("name")).or_else(|| text(m.get("id"))))
            .collect(),
        loads_sessions,
    }
}

fn offer(reply: &Value) -> Offer {
    let names = |choices: Vec<Choice>| -> Vec<String> {
        choices
            .into_iter()
            .map(|c| if c.name.is_empty() { c.id } else { c.name })
            .collect()
    };
    Offer {
        models: names(options::models(reply)),
        current_model: options::current_model(reply),
        modes: names(options::mode_ids(reply)),
        efforts: names(options::efforts(reply)),
    }
}

#[cfg(test)]
#[path = "diagnose_tests.rs"]
mod diagnose_tests;
