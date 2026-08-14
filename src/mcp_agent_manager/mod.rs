//! Agent-manager MCP.
//!
//! Agent MCP processes are short-lived children of the runner, so they cannot call the
//! in-process [`RunnerRegistry`] directly. This module is the small Unix-socket RPC
//! between those MCP processes and the registry, plus the JSON-RPC/stdio MCP facade the
//! runner actually talks to.
//!
//! The split: [`bridge`] is the child half (stdio in, socket out), everything else is the
//! opman half. [`dispatch`] is the one type both halves agree on — a turn cannot be
//! started without a model and an effort.

mod bridge;
mod dispatch;
mod endpoint;
#[cfg(test)]
mod fake_runner;
mod ops;
mod options;
mod permission;
mod queue;
mod request;
mod tools;
mod turn;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

use crate::runner::RunnerRegistry;
use dispatch::Dispatch;
use queue::QueuedMessage;
use request::{ManagerRequest, Op};

pub use bridge::run_bridge;
pub use endpoint::socket_path;

pub(crate) const SOCKET_ENV: &str = "OPMAN_AGENT_MANAGER_SOCKET";
const SESSION_ENV: &str = "OPENCODE_SESSION_ID";

#[derive(Clone)]
struct ManagerState {
    registry: Arc<RunnerRegistry>,
    parents: Arc<Mutex<HashMap<String, String>>>,
    queues: Arc<Mutex<HashMap<String, Vec<QueuedMessage>>>>,
}

/// Start the in-process manager endpoint and return its child-visible socket path.
pub fn spawn(registry: Arc<RunnerRegistry>) -> Result<PathBuf> {
    let path = socket_path();
    let endpoint = endpoint::Endpoint::bind(path.clone())?;
    let state = ManagerState {
        registry,
        parents: Arc::new(Mutex::new(HashMap::new())),
        queues: Arc::new(Mutex::new(HashMap::new())),
    };
    let worker_state = state.clone();
    tokio::spawn(async move { queue::worker(worker_state).await });
    tokio::spawn(async move {
        let mut endpoint = endpoint;
        let mut ticker = tokio::time::interval(endpoint::SUPERVISOR_INTERVAL);
        loop {
            if !endpoint.is_bound() {
                ticker.tick().await;
                endpoint = endpoint::supervise(endpoint);
                continue;
            }

            let parts = match endpoint.take_bound() {
                Some(parts) => parts,
                None => {
                    tracing::error!("agent manager endpoint lost its listener");
                    return;
                }
            };
            let (identity, listener) = parts.split();
            tokio::select! {
                result = listener.accept() => {
                    endpoint = endpoint::Endpoint::from_parts(identity, listener);
                    let (stream, _) = match result {
                        Ok(value) => value,
                        Err(error) => {
                            tracing::warn!(%error, "agent manager socket accept failed");
                            continue;
                        }
                    };
                    let state = state.clone();
                    tokio::spawn(async move {
                        if let Err(error) = handle_socket_connection(stream, state).await {
                            tracing::debug!(%error, "agent manager request failed");
                        }
                    });
                }
                _ = ticker.tick() => {
                    endpoint = endpoint::Endpoint::from_parts(identity, listener);
                    endpoint = endpoint::supervise(endpoint);
                }
            }
        }
    });
    Ok(path)
}

async fn handle_socket_connection(mut stream: UnixStream, state: ManagerState) -> Result<()> {
    let mut line = String::new();
    BufReader::new(&mut stream).read_line(&mut line).await?;
    if line.trim().is_empty() {
        return Ok(());
    }
    let request: ManagerRequest = serde_json::from_str(line.trim())?;
    let response = match handle_manager_request(&state, request).await {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    };
    stream
        .write_all(serde_json::to_string(&response)?.as_bytes())
        .await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;
    Ok(())
}

async fn handle_manager_request(state: &ManagerState, request: ManagerRequest) -> Result<Value> {
    let op = request.op()?;
    // Handled before the directory check: this one comes from the MCP proxy rather than a
    // tool call, and carries no project.
    if op == Op::AuthRequired {
        tracing::info!(server = ?request.server, "mcp server needs the user to log in");
        return Ok(json!({ "acknowledged": true }));
    }
    let directory = request
        .directory
        .as_deref()
        .context("agent manager requires a project directory")?;
    let source = request.source_session.clone().unwrap_or_default();
    let runner = request.runner_kind()?;
    let delivery = request.delivery_mode()?;

    match op {
        Op::Send => turn::send(state, &request, directory, source, runner, delivery).await,
        Op::Start => turn::start(state, &request, directory, &source, runner, delivery).await,
        Op::Progress => {
            let target = resolve_target(state, request.target.as_deref(), &source).await?;
            let mut progress = state.registry.progress(&target, directory).await?;
            let queued = state
                .queues
                .lock()
                .await
                .get(&target)
                .map(Vec::len)
                .unwrap_or(0);
            progress["queued_messages"] = json!(queued);
            Ok(progress)
        }
        Op::Options => {
            let kind = runner.unwrap_or_else(|| state.registry.default_kind());
            options::runner_options(&state.registry, kind, directory, request.filter.as_deref())
                .await
        }
        Op::Abort => {
            let target = resolve_target(state, request.target.as_deref(), &source).await?;
            ops::abort(state, &target, directory).await
        }
        Op::List => ops::list(state, directory).await,
        Op::Wait => {
            let target = resolve_target(state, request.target.as_deref(), &source).await?;
            ops::wait(state, &target, directory, request.timeout).await
        }
        // Answered above, before the project directory this arm cannot supply.
        Op::AuthRequired => Ok(json!({ "acknowledged": true })),
    }
}

/// The agent a call is about: the one named, or the parent that started this session.
async fn resolve_target(
    state: &ManagerState,
    target: Option<&str>,
    source: &str,
) -> Result<String> {
    if let Some(target) = target.filter(|value| !value.trim().is_empty() && *value != "parent") {
        return Ok(target.to_string());
    }
    if source.is_empty() {
        anyhow::bail!("target is required when the MCP session has no parent")
    }
    state
        .parents
        .lock()
        .await
        .get(source)
        .cloned()
        .context("parent session is unknown; pass an explicit target agent id")
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;

#[cfg(test)]
#[path = "live_support.rs"]
mod live_support;

#[cfg(test)]
#[path = "live_tests.rs"]
mod live;

#[cfg(test)]
#[path = "live_options_tests.rs"]
mod live_options;
