//! MCP code server — runs as `opman mcp-code [project_path]`.
//!
//! Exposes one tool, `execute`, that runs a short JavaScript program against every other
//! MCP server opman knows about. Instead of the model calling ten tools one round trip at
//! a time and carrying every intermediate payload, it writes one script: call tools with
//! `await tools.<server>.<tool>(input)`, run independent calls in parallel with
//! `Promise.all`, filter in code, and return only what matters.
//!
//! The servers are reached as an MCP *client* (see [`client`]): stdio servers are
//! launched, remote ones through `opman mcp-proxy` so credentials stay in opman. The
//! script itself runs in QuickJS (see [`sandbox`]) with no filesystem, network, or process
//! access except through those tools.
//!
//! Like the ask server, calls are spawned rather than awaited in the read loop: a script
//! can run for minutes, and blocking the pipe would stall the `notifications/cancelled`
//! that is meant to stop it.

mod catalog;
mod client;
mod describe;
mod execute;
mod names;
mod result;
mod rpc;
mod sandbox;
mod schema;
mod search;
#[cfg(test)]
mod testing;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader};
use tokio::task::JoinSet;

use client::Pool;
use rpc::{Out, RpcRequest};
use sandbox::Cancel;

/// The session whose servers the inner terminal/neovim tools belong to.
const SESSION_ENV: &str = "OPENCODE_SESSION_ID";
/// How long a closing pipe waits for answered calls to write their result.
const SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_millis(250);

pub async fn run_mcp_code_bridge(project_path: PathBuf) -> anyhow::Result<()> {
    let directory = std::fs::canonicalize(&project_path)
        .unwrap_or(project_path)
        .to_string_lossy()
        .into_owned();
    let session = std::env::var(SESSION_ENV).ok().filter(|id| !id.is_empty());
    // Every built-in is visible to the registry here; `eligible` decides what is mounted.
    let registry = crate::mcp_registry::McpRegistry::load(crate::mcp_registry::BuiltinFlags::ALL);
    let pool = client::pool_from_registry(&registry, &directory, session.as_deref());
    serve(
        Arc::new(pool),
        tokio::io::stdin(),
        Arc::new(tokio::sync::Mutex::new(tokio::io::stdout())),
    )
    .await
}

/// Calls in flight, so `notifications/cancelled` can stop the one it names.
#[derive(Default)]
struct Running(Mutex<HashMap<String, Cancel>>);

impl Running {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Cancel>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

async fn serve<R, W>(pool: Arc<Pool>, reader: R, out: Out<W>) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut tasks = JoinSet::new();
    let running = Arc::new(Running::default());
    loop {
        while tasks.try_join_next().is_some() {}
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        match serde_json::from_str::<RpcRequest>(text) {
            Ok(request) => dispatch(request, &pool, &out, &running, &mut tasks).await,
            Err(error) => {
                let message = format!("Parse error: {error}");
                rpc::write(&out, &rpc::error(Value::Null, -32700, &message)).await;
            }
        }
    }
    // Calls already answered still owe their reply; anything left after the grace
    // period can never be read now and is stopped.
    let drain = async { while tasks.join_next().await.is_some() {} };
    if tokio::time::timeout(SHUTDOWN_GRACE, drain).await.is_err() {
        for cancel in running.lock().values() {
            cancel.cancel();
        }
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
    Ok(())
}

async fn dispatch<W>(
    request: RpcRequest,
    pool: &Arc<Pool>,
    out: &Out<W>,
    running: &Arc<Running>,
    tasks: &mut JoinSet<()>,
) where
    W: AsyncWrite + Unpin + Send + 'static,
{
    if request.id.is_null() && request.method != "notifications/cancelled" {
        return;
    }
    match request.method.as_str() {
        "initialize" => {
            let info = json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "opman-code", "version": env!("CARGO_PKG_VERSION") },
            });
            rpc::write(out, &rpc::result(request.id, info)).await;
        }
        "notifications/initialized" => {}
        "ping" => rpc::write(out, &rpc::result(request.id, json!({}))).await,
        "notifications/cancelled" => {
            let id = request.params.as_ref().and_then(|p| p.get("requestId"));
            if let Some(cancel) = id.and_then(|id| running.lock().get(&id.to_string()).cloned()) {
                cancel.cancel();
            }
        }
        "tools/list" => {
            let (pool, out) = (pool.clone(), out.clone());
            tasks.spawn(async move {
                let catalog = pool.catalog().await;
                let listing = json!({ "tools": [execute::definition(&catalog)] });
                rpc::write(&out, &rpc::result(request.id, listing)).await;
            });
        }
        "tools/call" => {
            let (pool, out, running) = (pool.clone(), out.clone(), running.clone());
            tasks.spawn(async move { tool_call(request, pool, out, running).await });
        }
        other => {
            let message = format!("Method not found: {other}");
            rpc::write(out, &rpc::error(request.id, -32601, &message)).await;
        }
    }
}

async fn tool_call<W>(request: RpcRequest, pool: Arc<Pool>, out: Out<W>, running: Arc<Running>)
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    let params = request.params.as_ref();
    let named = params.and_then(|p| p.get("name")).and_then(Value::as_str);
    let token = params
        .and_then(|p| p.pointer("/_meta/progressToken"))
        .cloned();
    let outcome = match (
        named,
        execute::parse(params.and_then(|p| p.get("arguments"))),
    ) {
        (Some(execute::TOOL_NAME), Ok(input)) => {
            let cancel = Cancel::default();
            let key = request.id.to_string();
            running.lock().insert(key.clone(), cancel.clone());
            let work = execute::call(&pool, input, cancel);
            let outcome = tokio::select! {
                outcome = work => outcome,
                never = rpc::tick_until_dropped(&out, token) => match never {},
            };
            running.lock().remove(&key);
            outcome
        }
        (Some(execute::TOOL_NAME), Err(complaint)) => execute::failure(&complaint),
        (other, _) => execute::failure(&format!("Unknown tool: {}", other.unwrap_or_default())),
    };
    rpc::write(&out, &rpc::result(request.id, outcome)).await;
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
