//! Agent-manager MCP.
//!
//! Agent MCP processes are short-lived children of the runner, so they cannot
//! call the in-process [`RunnerRegistry`] directly.  This module provides the
//! small Unix-socket RPC between those MCP processes and the registry, and a
//! JSON-RPC/stdio MCP facade on top of it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

use crate::runner::{RunnerKind, RunnerRegistry};

const SOCKET_ENV: &str = "OPMAN_AGENT_MANAGER_SOCKET";
const SESSION_ENV: &str = "OPENCODE_SESSION_ID";

#[derive(Clone)]
struct ManagerState {
    registry: Arc<RunnerRegistry>,
    parents: Arc<Mutex<HashMap<String, String>>>,
    queues: Arc<Mutex<HashMap<String, Vec<QueuedMessage>>>>,
}

#[derive(Clone, Debug)]
struct QueuedMessage {
    id: String,
    source: String,
    target: String,
    directory: String,
    runner: Option<RunnerKind>,
    body: Value,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ManagerRequest {
    op: String,
    #[serde(default)]
    source_session: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    runner: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    delivery: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<Value>,
    id: Value,
}

/// Start the in-process manager endpoint and return its child-visible socket path.
pub fn spawn(registry: Arc<RunnerRegistry>) -> Result<PathBuf> {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = std::os::unix::net::UnixListener::bind(&path)
        .with_context(|| format!("failed to bind agent manager socket at {}", path.display()))?;
    listener
        .set_nonblocking(true)
        .context("failed to configure agent manager socket")?;
    let listener =
        UnixListener::from_std(listener).context("failed to initialize agent manager socket")?;
    let state = ManagerState {
        registry,
        parents: Arc::new(Mutex::new(HashMap::new())),
        queues: Arc::new(Mutex::new(HashMap::new())),
    };
    let worker_state = state.clone();
    tokio::spawn(async move { queue_worker(worker_state).await });
    tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
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
    });
    Ok(path)
}

/// Stable per-process path used to pass the endpoint to runner processes that
/// are started before the registry itself has been assembled.
pub fn socket_path() -> PathBuf {
    std::env::temp_dir().join(format!("opman-agent-manager-{}.sock", std::process::id()))
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
    let directory = request
        .directory
        .as_deref()
        .context("agent manager requires a project directory")?;
    let source = request.source_session.unwrap_or_default();
    let runner = parse_runner(request.runner.as_deref())?;
    let delivery = parse_delivery(request.delivery.as_deref())?;

    match request.op.as_str() {
        "send" => {
            let target = resolve_target(state, request.target.as_deref(), &source).await?;
            let body = message_body(
                request
                    .message
                    .as_deref()
                    .context("agent_send requires 'message'")?,
                request.model.as_deref(),
                request.provider.as_deref(),
            );
            deliver(
                state,
                QueuedMessage {
                    id: new_id("msg"),
                    source: source.clone(),
                    target: target.clone(),
                    directory: directory.to_string(),
                    runner,
                    body,
                },
                delivery,
            )
            .await
        }
        "start" => {
            let kind = match runner {
                Some(kind) => kind,
                None if !source.is_empty() => state.registry.runner_for(&source).await,
                None => state.registry.default_kind(),
            };
            let session = state
                .registry
                .create_session(
                    kind.clone(),
                    directory,
                    request.title.as_deref().unwrap_or("Agent session"),
                )
                .await?;
            if !source.is_empty() {
                state
                    .parents
                    .lock()
                    .await
                    .insert(session.id.clone(), source.clone());
            }
            let Some(message) = request
                .message
                .as_deref()
                .filter(|text| !text.trim().is_empty())
            else {
                return Ok(json!({
                    "session_id": session.id,
                    "runner": kind,
                    "delivery": "none",
                }));
            };
            let delivery = delivery.unwrap_or(Delivery::Immediate);
            let result = deliver(
                state,
                QueuedMessage {
                    id: new_id("msg"),
                    source: source.clone(),
                    target: session.id.clone(),
                    directory: directory.to_string(),
                    runner: None,
                    body: message_body(
                        message,
                        request.model.as_deref(),
                        request.provider.as_deref(),
                    ),
                },
                Some(delivery),
            )
            .await?;
            let mut result = result;
            result["session_id"] = Value::String(session.id);
            result["runner"] = serde_json::to_value(kind)?;
            Ok(result)
        }
        "progress" => {
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
        _ => anyhow::bail!("unknown agent manager operation: {}", request.op),
    }
}

async fn deliver(
    state: &ManagerState,
    message: QueuedMessage,
    delivery: Option<Delivery>,
) -> Result<Value> {
    if delivery == Some(Delivery::Queued) {
        let id = message.id.clone();
        state
            .queues
            .lock()
            .await
            .entry(message.target.clone())
            .or_default()
            .push(message);
        return Ok(json!({ "message_id": id, "delivery": "queued" }));
    }
    let outcome = state
        .registry
        .send_message(
            &message.target,
            &message.directory,
            message.runner,
            message.body,
        )
        .await?;
    if outcome.switched && !message.source.is_empty() {
        state
            .parents
            .lock()
            .await
            .insert(outcome.session_id.clone(), message.source.clone());
    }
    Ok(json!({
        "message_id": message.id,
        "delivery": "immediate",
        "session_id": outcome.session_id,
        "runner": outcome.runner,
        "switched": outcome.switched,
        "response": outcome.response,
    }))
}

async fn queue_worker(state: ManagerState) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
    loop {
        interval.tick().await;
        let targets: Vec<String> = state.queues.lock().await.keys().cloned().collect();
        for target in targets {
            let Some(first) = state
                .queues
                .lock()
                .await
                .get(&target)
                .and_then(|q| q.first())
                .cloned()
            else {
                continue;
            };
            let Ok(progress) = state.registry.progress(&target, &first.directory).await else {
                continue;
            };
            if progress.get("busy").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let message = {
                let mut queues = state.queues.lock().await;
                queues.get_mut(&target).and_then(|queue| {
                    if queue.is_empty() {
                        None
                    } else {
                        Some(queue.remove(0))
                    }
                })
            };
            let Some(message) = message else { continue };
            if state
                .registry
                .send_message(
                    &message.target,
                    &message.directory,
                    message.runner.clone(),
                    message.body.clone(),
                )
                .await
                .is_err()
            {
                state
                    .queues
                    .lock()
                    .await
                    .entry(target)
                    .or_default()
                    .insert(0, message);
            }
        }
    }
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Delivery {
    Immediate,
    Queued,
}

fn parse_delivery(value: Option<&str>) -> Result<Option<Delivery>> {
    match value
        .unwrap_or("immediate")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "immediate" | "steer" => Ok(Some(Delivery::Immediate)),
        "queued" | "next_turn" | "next-turn" => Ok(Some(Delivery::Queued)),
        other => anyhow::bail!("delivery must be 'immediate' or 'queued', got '{other}'"),
    }
}

fn parse_runner(value: Option<&str>) -> Result<Option<RunnerKind>> {
    value
        .map(|value| RunnerKind::parse(value).with_context(|| format!("unknown runner '{value}'")))
        .transpose()
}

fn message_body(message: &str, model: Option<&str>, provider: Option<&str>) -> Value {
    let mut body = json!({ "parts": [{ "type": "text", "text": message }] });
    if let Some(model) = model.filter(|value| !value.trim().is_empty()) {
        body["model"] = json!({
            "providerID": provider.unwrap_or_default(),
            "modelID": model,
        });
    }
    body
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", rand::random::<u128>())
}

/// Run the agent-manager MCP stdio bridge from a runner child process.
pub async fn run_bridge(project_path: PathBuf) -> Result<()> {
    let socket = std::env::var(SOCKET_ENV).context("agent manager socket is not configured")?;
    let source = std::env::var(SESSION_ENV).ok();
    let project_path = std::fs::canonicalize(&project_path).unwrap_or(project_path);
    let stdout = Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
    run_bridge_over(
        tokio::io::stdin(),
        stdout,
        Arc::new(PathBuf::from(socket)),
        source,
        project_path,
    )
    .await
}

async fn run_bridge_over<R, W>(
    reader: R,
    stdout: Arc<tokio::sync::Mutex<W>>,
    socket: Arc<PathBuf>,
    source: Option<String>,
    project_path: PathBuf,
) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let request: RpcRequest = match serde_json::from_str(line.trim()) {
            Ok(request) => request,
            Err(error) => {
                write_rpc(&stdout, &json!({ "jsonrpc": "2.0", "error": { "code": -32700, "message": error.to_string() }, "id": null })).await;
                continue;
            }
        };
        match request.method.as_str() {
            "initialize" => write_rpc(&stdout, &json!({
                "jsonrpc": "2.0", "id": request.id, "result": {
                    "protocolVersion": "2024-11-05", "capabilities": { "tools": {} },
                    "serverInfo": { "name": "opman-agent-manager", "version": env!("CARGO_PKG_VERSION") }
                }
            })).await,
            "notifications/initialized" => {}
            "tools/list" => write_rpc(&stdout, &json!({ "jsonrpc": "2.0", "id": request.id, "result": { "tools": tool_definitions() } })).await,
            "tools/call" => {
                let socket = socket.clone();
                let stdout = stdout.clone();
                let source = source.clone();
                let directory = project_path.to_string_lossy().to_string();
                tokio::spawn(async move {
                    let result = call_tool(&socket, request.params, source.as_deref(), &directory).await;
                    let response = match result {
                        Ok(value) => json!({ "jsonrpc": "2.0", "id": request.id, "result": { "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }] } }),
                        Err(error) => json!({ "jsonrpc": "2.0", "id": request.id, "result": { "isError": true, "content": [{ "type": "text", "text": error.to_string() }] } }),
                    };
                    write_rpc(&stdout, &response).await;
                });
            }
            other => write_rpc(&stdout, &json!({ "jsonrpc": "2.0", "id": request.id, "error": { "code": -32601, "message": format!("Method not found: {other}") } })).await,
        }
    }
    Ok(())
}

async fn write_rpc<W: AsyncWrite + Unpin>(stdout: &Arc<tokio::sync::Mutex<W>>, value: &Value) {
    if let Ok(text) = serde_json::to_string(value) {
        let mut stdout = stdout.lock().await;
        let _ = stdout.write_all(text.as_bytes()).await;
        let _ = stdout.write_all(b"\n").await;
        let _ = stdout.flush().await;
    }
}

async fn call_tool(
    socket: &Path,
    params: Option<Value>,
    source: Option<&str>,
    directory: &str,
) -> Result<Value> {
    let params = params.unwrap_or_else(|| json!({}));
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .context("missing tool name")?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let request = ManagerRequest {
        op: match name {
            "agent_send" => "send",
            "agent_start" => "start",
            "agent_progress" => "progress",
            other => anyhow::bail!("unknown tool: {other}"),
        }
        .into(),
        source_session: source.map(str::to_string),
        target: args
            .get("to")
            .or_else(|| args.get("agent_id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        directory: Some(directory.to_string()),
        runner: args
            .get("runner")
            .and_then(Value::as_str)
            .map(str::to_string),
        model: args
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider: args
            .get("provider")
            .and_then(Value::as_str)
            .map(str::to_string),
        title: args
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string),
        message: args
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string),
        delivery: args
            .get("delivery")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    let mut stream = UnixStream::connect(socket)
        .await
        .with_context(|| format!("failed to connect to agent manager at {}", socket.display()))?;
    stream
        .write_all(serde_json::to_string(&request)?.as_bytes())
        .await?;
    stream.write_all(b"\n").await?;
    stream.shutdown().await?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).await?;
    let response: Value = serde_json::from_str(response.trim())?;
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        let error = response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("agent manager request failed")
            .to_string();
        anyhow::bail!(error);
    }
    Ok(response.get("data").cloned().unwrap_or(Value::Null))
}

fn tool_definitions() -> Value {
    json!([
        { "name": "agent_send", "description": "Send a message to the parent agent (omit 'to') or any agent id. delivery=immediate steers now; delivery=queued sends it on the target's next idle turn.", "inputSchema": { "type": "object", "properties": {
            "to": { "type": "string", "description": "Target agent id, or 'parent'. Omit to address the parent." },
            "message": { "type": "string" }, "delivery": { "type": "string", "enum": ["immediate", "queued"], "default": "immediate" },
            "runner": { "type": "string", "enum": ["opencode", "claude-code", "claude", "codex"] }, "model": { "type": "string" }, "provider": { "type": "string" }
        }, "required": ["message"] } },
        { "name": "agent_start", "description": "Start a new agent session on any available runner, optionally with a model and initial message.", "inputSchema": { "type": "object", "properties": {
            "message": { "type": "string" }, "title": { "type": "string" }, "runner": { "type": "string", "enum": ["opencode", "claude-code", "claude", "codex"] }, "model": { "type": "string" }, "provider": { "type": "string" }, "delivery": { "type": "string", "enum": ["immediate", "queued"], "default": "immediate" }
        } } },
        { "name": "agent_progress", "description": "Inspect another agent's current busy/idle state, recent transcript messages, runner, and queued message count.", "inputSchema": { "type": "object", "properties": { "agent_id": { "type": "string", "description": "Agent id, or 'parent'. Omit to ask about the parent." } } } }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_body_keeps_runner_neutral_model_fields() {
        let body = message_body("hello", Some("sonnet"), Some("anthropic"));
        assert_eq!(body["model"]["modelID"], "sonnet");
        assert_eq!(body["parts"][0]["text"], "hello");
    }

    #[test]
    fn delivery_aliases_are_supported() {
        assert_eq!(
            parse_delivery(Some("steer")).unwrap(),
            Some(Delivery::Immediate)
        );
        assert_eq!(
            parse_delivery(Some("next_turn")).unwrap(),
            Some(Delivery::Queued)
        );
        assert!(parse_delivery(Some("later")).is_err());
    }
}
