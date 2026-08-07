//! The child half: a JSON-RPC/stdio MCP server that forwards to opman's socket.
//!
//! It holds no state and makes no decisions. Every argument is passed through to the
//! manager, which is where the required model and effort are actually enforced — the bridge
//! must not second-guess a contract it would then have to keep in step.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

use super::request::ManagerRequest;
use super::{tools, SESSION_ENV, SOCKET_ENV};

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<Value>,
    id: Value,
}

/// Run the agent-manager MCP stdio bridge from a runner child process.
pub async fn run_bridge(project_path: PathBuf) -> Result<()> {
    let socket = std::env::var(SOCKET_ENV).context("agent manager socket is not configured")?;
    let source = std::env::var(SESSION_ENV).ok();
    let project_path = std::fs::canonicalize(&project_path).unwrap_or(project_path);
    let stdout = Arc::new(Mutex::new(tokio::io::stdout()));
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
    stdout: Arc<Mutex<W>>,
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
                write_rpc(
                    &stdout,
                    &json!({ "jsonrpc": "2.0", "error": { "code": -32700, "message": error.to_string() }, "id": null }),
                )
                .await;
                continue;
            }
        };
        dispatch_rpc(request, &stdout, &socket, source.as_deref(), &project_path).await;
    }
    Ok(())
}

async fn dispatch_rpc<W>(
    request: RpcRequest,
    stdout: &Arc<Mutex<W>>,
    socket: &Arc<PathBuf>,
    source: Option<&str>,
    project_path: &Path,
) where
    W: AsyncWrite + Unpin + Send + 'static,
{
    match request.method.as_str() {
        "initialize" => {
            write_rpc(
                stdout,
                &json!({ "jsonrpc": "2.0", "id": request.id, "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "opman-agent-manager", "version": env!("CARGO_PKG_VERSION") },
                }}),
            )
            .await
        }
        "notifications/initialized" => {}
        "tools/list" => {
            write_rpc(
                stdout,
                &json!({ "jsonrpc": "2.0", "id": request.id, "result": { "tools": tools::definitions() } }),
            )
            .await
        }
        // Spawned, not awaited: a steer to a busy agent can sit for a whole turn, and
        // blocking here would stall every later request on this one stdio pipe.
        "tools/call" => {
            let socket = socket.clone();
            let stdout = stdout.clone();
            let source = source.map(str::to_string);
            let directory = project_path.to_string_lossy().into_owned();
            tokio::spawn(async move {
                let result = call_tool(&socket, request.params, source.as_deref(), &directory).await;
                let response = match result {
                    Ok(value) => json!({ "jsonrpc": "2.0", "id": request.id, "result": {
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
                    }}),
                    Err(error) => json!({ "jsonrpc": "2.0", "id": request.id, "result": {
                        "isError": true,
                        "content": [{ "type": "text", "text": error.to_string() }],
                    }}),
                };
                write_rpc(&stdout, &response).await;
            });
        }
        other => {
            write_rpc(
                stdout,
                &json!({ "jsonrpc": "2.0", "id": request.id, "error": { "code": -32601, "message": format!("Method not found: {other}") } }),
            )
            .await
        }
    }
}

async fn write_rpc<W: AsyncWrite + Unpin>(stdout: &Arc<Mutex<W>>, value: &Value) {
    if let Ok(text) = serde_json::to_string(value) {
        let mut stdout = stdout.lock().await;
        let _ = stdout.write_all(text.as_bytes()).await;
        let _ = stdout.write_all(b"\n").await;
        let _ = stdout.flush().await;
    }
}

/// A tool's arguments as a manager request, then one round trip over the socket.
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
    let args = params.get("arguments").unwrap_or(&Value::Null);
    let request = to_request(name, args, source, directory)?;
    exchange(socket, &request).await
}

fn to_request(
    name: &str,
    args: &Value,
    source: Option<&str>,
    directory: &str,
) -> Result<ManagerRequest> {
    let text = |key: &str| args.get(key).and_then(Value::as_str).map(str::to_string);
    Ok(ManagerRequest {
        op: match name {
            "agent_send" => "send",
            "agent_start" => "start",
            "agent_progress" => "progress",
            "agent_runner_options" => "options",
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
        runner: text("runner"),
        model: text("model"),
        effort: text("effort"),
        provider: text("provider"),
        title: text("title"),
        message: text("message"),
        delivery: text("delivery"),
    })
}

async fn exchange(socket: &Path, request: &ManagerRequest) -> Result<Value> {
    let mut stream = UnixStream::connect(socket)
        .await
        .with_context(|| format!("failed to connect to agent manager at {}", socket.display()))?;
    stream
        .write_all(serde_json::to_string(request)?.as_bytes())
        .await?;
    stream.write_all(b"\n").await?;
    stream.shutdown().await?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).await?;
    let response: Value = serde_json::from_str(response.trim())?;
    if response.get("ok").and_then(Value::as_bool) != Some(true) {
        anyhow::bail!(response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("agent manager request failed")
            .to_string());
    }
    Ok(response.get("data").cloned().unwrap_or(Value::Null))
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod bridge_tests;
