//! WebSocket-based MCP (Model Context Protocol) server for the web UI.
//!
//! Exposes web terminal and editor tools to AI agents via JSON-RPC 2.0
//! over WebSocket. This bridges the gap between the TUI's Unix socket
//! MCP server (`src/mcp.rs`) and the web UI, allowing AI agents running
//! in web-spawned sessions to control terminals and the CodeMirror editor.
//!
//! ## Protocol
//!
//! Standard MCP over JSON-RPC 2.0:
//! - `initialize` — handshake with capabilities
//! - `tools/list` — enumerate available tools
//! - `tools/call` — invoke a tool
//!
//! ## Tools
//!
//! Terminal tools (backed by `WebPtyHandle`):
//! - `web_terminal_read` — read output from a web PTY
//! - `web_terminal_run` — write a command to a web PTY
//! - `web_terminal_list` — list active web PTYs
//! - `web_terminal_new` — spawn a new web PTY shell
//! - `web_terminal_close` — kill a web PTY
//!
//! Editor tools (emit SSE events to frontend):
//! - `web_editor_open` — open a file in the CodeMirror editor
//! - `web_editor_read` — read a file from disk
//! - `web_editor_list` — list files in the project directory
//!
//! ## Authentication
//!
//! JWT token passed as `?token=<jwt>` query parameter (same as SSE endpoints).

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use super::auth::check_auth_manual;
use super::error::WebError;
use super::types::{ServerState, SseTokenQuery, WebEvent};

// ── JSON-RPC 2.0 types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<serde_json::Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

// ── MCP constants ───────────────────────────────────────────────────

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "opman-web-mcp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// JSON-RPC error codes
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INTERNAL_ERROR: i64 = -32603;

// ── WebSocket handler ───────────────────────────────────────────────

/// WebSocket upgrade handler for MCP connections.
///
/// `GET /api/mcp/ws?token=<jwt>`
pub async fn websocket_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SseTokenQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, WebError> {
    if !check_auth_manual(&state, &headers, &params.token) {
        return Err(WebError::Unauthorized);
    }

    Ok(ws.on_upgrade(move |socket| handle_mcp_session(socket, state)))
}

/// Main MCP session loop — reads JSON-RPC requests, dispatches, sends responses.
async fn handle_mcp_session(socket: WebSocket, state: ServerState) {
    let (mut sender, mut receiver) = socket.split();

    debug!("MCP WebSocket client connected");

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => {
                debug!("MCP WebSocket client disconnected (close frame)");
                break;
            }
            Ok(_) => continue, // Ignore binary, ping, pong
            Err(e) => {
                warn!("MCP WebSocket error: {}", e);
                break;
            }
        };

        // Parse JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(
                    None,
                    PARSE_ERROR,
                    format!("JSON parse error: {}", e),
                );
                if let Ok(json) = serde_json::to_string(&resp) {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                continue;
            }
        };

        if request.jsonrpc != "2.0" {
            let resp = JsonRpcResponse::error(
                request.id.clone(),
                INVALID_REQUEST,
                "Expected jsonrpc: \"2.0\"",
            );
            if let Ok(json) = serde_json::to_string(&resp) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            continue;
        }

        // Dispatch method
        let response = dispatch_method(&state, &request).await;

        // Notifications (no id) don't get responses per JSON-RPC spec
        if request.id.is_none() {
            continue;
        }

        if let Ok(json) = serde_json::to_string(&response) {
            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    }

    debug!("MCP WebSocket session ended");
}

// ── Method dispatch ─────────────────────────────────────────────────

async fn dispatch_method(state: &ServerState, req: &JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(req),
        "initialized" => {
            // Notification — acknowledge silently
            JsonRpcResponse::success(req.id.clone(), serde_json::json!({}))
        }
        "tools/list" => handle_tools_list(req),
        "tools/call" => handle_tools_call(state, req).await,
        "ping" => JsonRpcResponse::success(req.id.clone(), serde_json::json!({})),
        _ => JsonRpcResponse::error(
            req.id.clone(),
            METHOD_NOT_FOUND,
            format!("Unknown method: {}", req.method),
        ),
    }
}

// ── MCP: initialize ─────────────────────────────────────────────────

fn handle_initialize(req: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        req.id.clone(),
        serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        }),
    )
}

// ── MCP: tools/list ─────────────────────────────────────────────────

fn handle_tools_list(req: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        req.id.clone(),
        serde_json::json!({
            "tools": web_mcp_tool_definitions()
        }),
    )
}

/// Tool definitions for the web MCP server.
fn web_mcp_tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "web_terminal_read",
            "description": "Read the terminal output from a web PTY terminal tab. Returns the raw terminal buffer content. Use `last_n` to limit to the most recent N lines.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "PTY ID (UUID). Use web_terminal_list to discover available IDs."
                    },
                    "last_n": {
                        "type": "number",
                        "description": "Return only the last N lines of the terminal output."
                    }
                },
                "required": ["id"]
            }
        },
        {
            "name": "web_terminal_run",
            "description": "Run a command in a web PTY terminal tab. The command is typed into the terminal and Enter is pressed. If a command is already running, send \"\\x03\" (Ctrl-C) to interrupt it first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "PTY ID (UUID). Use web_terminal_list to discover available IDs."
                    },
                    "command": {
                        "type": "string",
                        "description": "The command to run. Send \"\\x03\" (Ctrl-C) to interrupt a running command."
                    },
                    "wait": {
                        "type": "boolean",
                        "description": "If true, wait for command output to settle and return the terminal content. If false (default), fire-and-forget."
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Maximum time in seconds to wait when wait=true (default: 30)."
                    }
                },
                "required": ["id", "command"]
            }
        },
        {
            "name": "web_terminal_list",
            "description": "List all active web PTY terminal tabs. Returns an array of PTY IDs.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "web_terminal_new",
            "description": "Create a new web PTY terminal (shell). Returns the ID of the newly created PTY.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rows": {
                        "type": "number",
                        "description": "Terminal rows (default: 24)."
                    },
                    "cols": {
                        "type": "number",
                        "description": "Terminal columns (default: 80)."
                    }
                }
            }
        },
        {
            "name": "web_terminal_close",
            "description": "Close (kill) a web PTY terminal tab.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "PTY ID (UUID) to close."
                    }
                },
                "required": ["id"]
            }
        },
        {
            "name": "web_editor_open",
            "description": "Open a file in the web UI's CodeMirror editor. Optionally navigate to a specific line.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (absolute or relative to project root)."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number to navigate to (1-based)."
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "web_editor_read",
            "description": "Read the content of a file from disk. Returns the file content as text.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (absolute or relative to project root)."
                    },
                    "start_line": {
                        "type": "number",
                        "description": "Start line (1-based, inclusive). Omit to read from the beginning."
                    },
                    "end_line": {
                        "type": "number",
                        "description": "End line (1-based, inclusive). Omit to read to the end."
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "web_editor_list",
            "description": "List files in the project directory tree. Returns file paths relative to the project root.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Subdirectory to list (relative to project root). Defaults to project root."
                    },
                    "depth": {
                        "type": "number",
                        "description": "Maximum directory depth to traverse (default: 3)."
                    }
                }
            }
        }
    ])
}

// ── MCP: tools/call ─────────────────────────────────────────────────

async fn handle_tools_call(state: &ServerState, req: &JsonRpcRequest) -> JsonRpcResponse {
    let params = req.params.as_ref().cloned().unwrap_or(serde_json::json!({}));
    let tool_name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => {
            return JsonRpcResponse::error(
                req.id.clone(),
                INVALID_REQUEST,
                "Missing tool name in params.name",
            );
        }
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // Emit agent activity event
    let _ = state.event_tx.send(WebEvent::McpAgentActivity {
        tool: tool_name.clone(),
        active: true,
    });

    let result = match tool_name.as_str() {
        "web_terminal_read" => handle_terminal_read(state, &arguments).await,
        "web_terminal_run" => handle_terminal_run(state, &arguments).await,
        "web_terminal_list" => handle_terminal_list(state).await,
        "web_terminal_new" => handle_terminal_new(state, &arguments).await,
        "web_terminal_close" => handle_terminal_close(state, &arguments).await,
        "web_editor_open" => handle_editor_open(state, &arguments).await,
        "web_editor_read" => handle_editor_read(state, &arguments).await,
        "web_editor_list" => handle_editor_list(state, &arguments).await,
        _ => Err(format!("Unknown tool: {}", tool_name)),
    };

    // Emit agent activity off
    let _ = state.event_tx.send(WebEvent::McpAgentActivity {
        tool: tool_name,
        active: false,
    });

    match result {
        Ok(content) => JsonRpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": content
                }]
            }),
        ),
        Err(e) => JsonRpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": e
                }],
                "isError": true
            }),
        ),
    }
}

// ── Tool handlers: Terminal ─────────────────────────────────────────

async fn handle_terminal_read(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'id' argument")?;
    let last_n = args
        .get("last_n")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let output = state
        .pty_mgr
        .get_output(id)
        .await
        .ok_or_else(|| format!("PTY '{}' not found", id))?;

    let bytes = output.drain_new();
    if bytes.is_empty() {
        // If drain_new is empty, we've already read everything.
        // Return empty string to indicate no new output.
        return Ok("[no new output]".to_string());
    }

    let text = String::from_utf8_lossy(&bytes).to_string();
    match last_n {
        Some(n) => {
            let lines: Vec<&str> = text.lines().collect();
            let start = lines.len().saturating_sub(n);
            Ok(lines[start..].join("\n"))
        }
        None => Ok(text),
    }
}

async fn handle_terminal_run(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'id' argument")?;
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'command' argument")?;
    let wait = args.get("wait").and_then(|v| v.as_bool()).unwrap_or(false);
    let timeout_secs = args
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    // Write the command (with newline to execute)
    let mut data = command.as_bytes().to_vec();
    // Only append newline if it's not a control character (like Ctrl-C)
    if !command.starts_with('\x03') {
        data.push(b'\n');
    }

    let ok = state.pty_mgr.write(id, data).await;
    if !ok {
        return Err(format!("Failed to write to PTY '{}' (not found or dead)", id));
    }

    // Emit terminal focus event
    let _ = state.event_tx.send(WebEvent::McpTerminalFocus {
        id: id.to_string(),
    });

    if !wait {
        return Ok("Command sent".to_string());
    }

    // Wait for output to settle (poll until no new output for 500ms)
    let deadline =
        tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    let settle_duration = tokio::time::Duration::from_millis(500);
    let poll_interval = tokio::time::Duration::from_millis(100);
    let mut last_output_time = tokio::time::Instant::now();
    let mut accumulated = String::new();

    let output = state
        .pty_mgr
        .get_output(id)
        .await
        .ok_or_else(|| format!("PTY '{}' not found", id))?;

    // Small initial delay to let the command start producing output
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }

        let new_bytes = output.drain_new();
        if !new_bytes.is_empty() {
            accumulated.push_str(&String::from_utf8_lossy(&new_bytes));
            last_output_time = now;
        } else if now.duration_since(last_output_time) >= settle_duration {
            // Output has settled
            break;
        }

        tokio::time::sleep(poll_interval).await;
    }

    if accumulated.is_empty() {
        Ok("[command sent, no output captured]".to_string())
    } else {
        Ok(accumulated)
    }
}

async fn handle_terminal_list(state: &ServerState) -> Result<String, String> {
    let ids = state.pty_mgr.list().await;
    let result = serde_json::json!({
        "terminals": ids,
        "count": ids.len()
    });
    Ok(result.to_string())
}

async fn handle_terminal_new(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let rows = args
        .get("rows")
        .and_then(|v| v.as_u64())
        .unwrap_or(24) as u16;
    let cols = args
        .get("cols")
        .and_then(|v| v.as_u64())
        .unwrap_or(80) as u16;

    // Generate a UUID for the new PTY
    let id = format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        rand::random::<u32>(),
        rand::random::<u16>(),
        rand::random::<u16>(),
        rand::random::<u16>(),
        rand::random::<u64>() & 0xFFFFFFFFFFFF
    );

    // Get working directory from web state
    let working_dir = state
        .web_state
        .get_working_dir()
        .await
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    state
        .pty_mgr
        .spawn_shell(id.clone(), rows, cols, working_dir)
        .await
        .map_err(|e| format!("Failed to spawn PTY: {}", e))?;

    Ok(serde_json::json!({
        "id": id,
        "rows": rows,
        "cols": cols
    })
    .to_string())
}

async fn handle_terminal_close(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'id' argument")?;

    let ok = state.pty_mgr.kill(id).await;
    if ok {
        Ok(format!("PTY '{}' closed", id))
    } else {
        Err(format!("PTY '{}' not found", id))
    }
}

// ── Tool handlers: Editor ───────────────────────────────────────────

async fn handle_editor_open(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'path' argument")?;
    let line = args.get("line").and_then(|v| v.as_u64()).map(|v| v as u32);

    // Resolve path relative to project root if not absolute
    let resolved_path = if std::path::Path::new(path).is_absolute() {
        path.to_string()
    } else {
        match state.web_state.get_working_dir().await {
            Some(dir) => dir.join(path).to_string_lossy().to_string(),
            None => path.to_string(),
        }
    };

    // Verify file exists
    if !std::path::Path::new(&resolved_path).exists() {
        return Err(format!("File not found: {}", resolved_path));
    }

    // Emit SSE event to tell frontend to open the file
    let _ = state.event_tx.send(WebEvent::McpEditorOpen {
        path: resolved_path.clone(),
        line,
    });

    let msg = match line {
        Some(l) => format!("Opened '{}' at line {}", resolved_path, l),
        None => format!("Opened '{}'", resolved_path),
    };
    Ok(msg)
}

async fn handle_editor_read(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing required 'path' argument")?;
    let start_line = args.get("start_line").and_then(|v| v.as_u64());
    let end_line = args.get("end_line").and_then(|v| v.as_u64());

    // Resolve path
    let resolved_path = if std::path::Path::new(path).is_absolute() {
        std::path::PathBuf::from(path)
    } else {
        match state.web_state.get_working_dir().await {
            Some(dir) => dir.join(path),
            None => std::path::PathBuf::from(path),
        }
    };

    // Read the file
    let content = tokio::fs::read_to_string(&resolved_path)
        .await
        .map_err(|e| format!("Failed to read '{}': {}", resolved_path.display(), e))?;

    // Apply line range if specified
    match (start_line, end_line) {
        (Some(s), Some(e)) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = (s as usize).saturating_sub(1); // 1-based to 0-based
            let end = std::cmp::min(e as usize, lines.len());
            if start >= lines.len() {
                return Err(format!(
                    "start_line {} is past end of file ({} lines)",
                    s,
                    lines.len()
                ));
            }
            Ok(lines[start..end].join("\n"))
        }
        (Some(s), None) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = (s as usize).saturating_sub(1);
            if start >= lines.len() {
                return Err(format!(
                    "start_line {} is past end of file ({} lines)",
                    s,
                    lines.len()
                ));
            }
            Ok(lines[start..].join("\n"))
        }
        (None, Some(e)) => {
            let lines: Vec<&str> = content.lines().collect();
            let end = std::cmp::min(e as usize, lines.len());
            Ok(lines[..end].join("\n"))
        }
        (None, None) => Ok(content),
    }
}

async fn handle_editor_list(
    state: &ServerState,
    args: &serde_json::Value,
) -> Result<String, String> {
    let subpath = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let max_depth = args
        .get("depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(3) as usize;

    let base_dir = match state.web_state.get_working_dir().await {
        Some(dir) => {
            if subpath.is_empty() {
                dir
            } else {
                dir.join(subpath)
            }
        }
        None => return Err("No active project directory".to_string()),
    };

    if !base_dir.exists() {
        return Err(format!("Directory not found: {}", base_dir.display()));
    }

    // Walk the directory tree up to max_depth
    let mut files = Vec::new();
    collect_files(&base_dir, &base_dir, 0, max_depth, &mut files);

    let result = serde_json::json!({
        "root": base_dir.to_string_lossy(),
        "files": files,
        "count": files.len()
    });
    Ok(result.to_string())
}

/// Recursively collect file paths, skipping common ignore patterns.
fn collect_files(
    root: &std::path::Path,
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    files: &mut Vec<String>,
) {
    if depth > max_depth {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files and common ignore directories
        if name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "__pycache__"
            || name == "dist"
            || name == "build"
        {
            continue;
        }

        if path.is_dir() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            files.push(format!("{}/", rel.to_string_lossy()));
            collect_files(root, &path, depth + 1, max_depth, files);
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            files.push(rel.to_string_lossy().to_string());
        }
    }
}
