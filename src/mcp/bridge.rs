use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::task::JoinSet;

use super::tool_defs::mcp_tool_definitions;
use super::tools::handle_tool_call;
use super::types::socket_path_for_project;

// ─── MCP stdio bridge (runs as `opman --mcp <project_path>`) ─────

/// MCP JSON-RPC request (subset we handle).
#[derive(Debug, Deserialize)]
struct McpJsonRpcRequest {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
    id: serde_json::Value,
}

/// Run the MCP stdio bridge: read JSON-RPC from stdin, forward to socket, write response to stdout.
///
/// Tool calls (`tools/call`) are dispatched **concurrently**: each call is
/// spawned as a separate tokio task, and responses are written back with the
/// correct `id` as they complete — not in request order.  This allows the MCP
/// client (e.g. opencode) to issue parallel tool calls that execute at the same
/// time (subject to per-resource locking in the socket server layer).
///
/// Non-tool methods (initialize, tools/list) are still handled inline since
/// they are cheap and ordering-sensitive (initialize must complete before
/// tools/list).
///
/// This function is designed to **never exit** on transient errors. Only a
/// genuine EOF on stdin (the MCP client closed the pipe) causes a clean exit.
/// All stdout write failures are swallowed — the individual request is lost
/// but the bridge stays alive so subsequent requests still work.
pub async fn run_mcp_bridge(project_path: PathBuf) -> anyhow::Result<()> {
    let sock_path = Arc::new(socket_path_for_project(&project_path));
    // Read session ID from env var set by opencode PTY spawn, so all
    // socket requests route to the correct per-session resources.
    let session_id: Arc<Option<String>> = Arc::new(std::env::var("OPENCODE_SESSION_ID").ok());
    // Shared stdout writer protected by a tokio Mutex so concurrent tasks can
    // write responses without interleaving.
    let stdout: Arc<tokio::sync::Mutex<tokio::io::Stdout>> =
        Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
    run_bridge_over(tokio::io::stdin(), stdout, sock_path, session_id).await
}

/// Core bridge read-loop, parameterized over the input reader and output writer
/// so it can be driven with in-memory buffers in tests. Behavior is identical
/// to `run_mcp_bridge` using real stdin/stdout.
async fn run_bridge_over<R, W>(
    reader: R,
    stdout: Arc<tokio::sync::Mutex<W>>,
    sock_path: Arc<PathBuf>,
    session_id: Arc<Option<String>>,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut reader = BufReader::new(reader);

    let mut line = String::new();
    let mut calls = JoinSet::new();
    loop {
        while calls.try_join_next().is_some() {}
        line.clear();
        let n = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(e) => {
                eprintln!("MCP bridge stdin read error: {}", e);
                continue;
            }
        };
        if n == 0 {
            break; // EOF — client closed the pipe
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let rpc_req: McpJsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                write_jsonrpc_stdout(&stdout, &parse_error_response(&e.to_string())).await;
                continue;
            }
        };

        let _ = rpc_req.jsonrpc; // consumed for deserialization

        match rpc_req.method.as_str() {
            "initialize" => {
                write_jsonrpc_stdout(&stdout, &initialize_response(&rpc_req.id)).await;
            }
            "notifications/initialized" => {
                // Client acknowledgment, no response needed
                continue;
            }
            "tools/list" => {
                write_jsonrpc_stdout(&stdout, &tools_list_response(&rpc_req.id)).await;
            }
            "tools/call" => {
                // Spawn tool call concurrently — does not block the stdin reader
                let sock = Arc::clone(&sock_path);
                let sid = Arc::clone(&session_id);
                let out = Arc::clone(&stdout);
                let id = rpc_req.id.clone();
                let params = rpc_req.params;
                calls.spawn(async move {
                    let result = handle_tool_call(&sock, params, sid.as_deref()).await;
                    let response = tool_call_response(result, &id);
                    write_jsonrpc_stdout(&out, &response).await;
                });
            }
            _ => {
                write_jsonrpc_stdout(
                    &stdout,
                    &method_not_found_response(&rpc_req.method, &rpc_req.id),
                )
                .await;
            }
        }
    }

    while calls.join_next().await.is_some() {}

    Ok(())
}

/// Build the JSON-RPC response for an `initialize` request.
fn initialize_response(id: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "opman-terminal",
                "version": "1.0.0"
            }
        },
        "id": id
    })
}

/// Build the JSON-RPC response for a `tools/list` request.
fn tools_list_response(id: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "result": {
            "tools": mcp_tool_definitions()
        },
        "id": id
    })
}

/// Build the JSON-RPC response for an unknown method.
fn method_not_found_response(method: &str, id: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "error": { "code": -32601, "message": format!("Method not found: {}", method) },
        "id": id
    })
}

/// Build the JSON-RPC response for a JSON parse error.
fn parse_error_response(msg: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "error": { "code": -32700, "message": format!("Parse error: {}", msg) },
        "id": null
    })
}

/// Build the JSON-RPC response wrapping a tool-call result.
fn tool_call_response(
    result: anyhow::Result<serde_json::Value>,
    id: &serde_json::Value,
) -> serde_json::Value {
    match result {
        Ok(content) => serde_json::json!({
            "jsonrpc": "2.0",
            "result": {
                "content": content
            },
            "id": id
        }),
        Err(e) => serde_json::json!({
            "jsonrpc": "2.0",
            "result": {
                "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                "isError": true
            },
            "id": id
        }),
    }
}

/// Write a JSON-RPC response to a shared stdout (tokio Mutex-protected).
/// The entire write (json + newline + flush) is atomic w.r.t. the lock so
/// concurrent tasks never interleave their output.
async fn write_jsonrpc_stdout<W: AsyncWrite + Unpin>(
    stdout: &Arc<tokio::sync::Mutex<W>>,
    resp: &serde_json::Value,
) {
    let json = match serde_json::to_string(resp) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("MCP bridge: failed to serialize response: {}", e);
            return;
        }
    };
    let mut out = stdout.lock().await;
    if let Err(e) = out.write_all(json.as_bytes()).await {
        eprintln!("MCP bridge: stdout write error: {}", e);
        return;
    }
    if let Err(e) = out.write_all(b"\n").await {
        eprintln!("MCP bridge: stdout write error: {}", e);
        return;
    }
    if let Err(e) = out.flush().await {
        eprintln!("MCP bridge: stdout flush error: {}", e);
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod bridge_tests;

#[cfg(test)]
#[path = "bridge_loop_tests.rs"]
mod bridge_loop_tests;

#[cfg(test)]
#[path = "bridge_response_tests.rs"]
mod bridge_response_tests;
