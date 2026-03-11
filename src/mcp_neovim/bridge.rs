/// MCP Neovim server — runs as `opman --mcp-nvim <project_path>`
///
/// Exposes tools for interacting with the embedded Neovim instance:
///
/// **File & buffer:**
///   - `neovim_open`       — open a file (optionally at a line)
///   - `neovim_read`       — read lines from the current buffer
///   - `neovim_command`    — execute a Vim ex-command
///   - `neovim_buffers`    — list all loaded buffers
///   - `neovim_info`       — current buffer name, cursor position, line count
///   - `neovim_write`      — save current buffer or all buffers
///   - `neovim_diff`       — unsaved changes as a unified diff
///
/// **LSP:**
///   - `neovim_diagnostics` — LSP errors/warnings (buffer or project-wide)
///   - `neovim_definition`  — go-to-definition of symbol at position
///   - `neovim_references`  — find all references to symbol at position
///   - `neovim_hover`       — type/hover info for symbol at position
///   - `neovim_symbols`     — document or workspace symbol search
///   - `neovim_code_actions` — list available code actions at cursor
///
/// **Dev flow:**
///   - `neovim_eval`       — execute arbitrary Lua code in Neovim
///   - `neovim_grep`       — search project files via vimgrep
///
/// **Editing:**
///   - `neovim_edit`      — replace a range of lines with new content
///   - `neovim_undo`      — undo/redo changes in the buffer
///
/// **LSP refactoring:**
///   - `neovim_rename`    — rename a symbol across the project
///   - `neovim_format`    — format the buffer using LSP formatter
///   - `neovim_signature` — get function signature help at position
///
/// Like the terminal MCP bridge, this forwards requests over the project's
/// Unix socket to the manager process, which executes them against the
/// Neovim RPC socket.
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp;

use super::dispatch::handle_tool_call;
use super::tools::tool_definitions;

// ─── JSON-RPC types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(super) struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    pub id: serde_json::Value,
}

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Run the MCP neovim stdio bridge: read JSON-RPC from stdin, forward to
/// socket, write response to stdout.
///
/// Tool calls (`tools/call`) are dispatched **concurrently**: each call is
/// spawned as a separate tokio task so the MCP client can issue parallel
/// tool calls (e.g. reading two different files at once).  Responses are
/// written back with the correct `id` as they complete.  The socket server
/// layer provides per-file serialization for neovim operations.
pub async fn run_mcp_neovim_bridge(project_path: PathBuf) -> anyhow::Result<()> {
    let sock_path = Arc::new(mcp::socket_path_for_project(&project_path));
    // Read session ID from env var set by opencode PTY spawn, so all
    // socket requests route to the correct per-session resources.
    let session_id: Arc<Option<String>> = Arc::new(std::env::var("OPENCODE_SESSION_ID").ok());
    let stdin = tokio::io::stdin();
    let stdout: Arc<tokio::sync::Mutex<tokio::io::Stdout>> =
        Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let n = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(e) => {
                eprintln!("MCP neovim bridge stdin read error: {}", e);
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

        let req: McpRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) },
                    "id": null
                });
                write_response_shared(&stdout, &resp).await;
                continue;
            }
        };

        match req.method.as_str() {
            "initialize" => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": { "tools": {} },
                        "serverInfo": {
                            "name": "opman-neovim",
                            "version": "1.0.0"
                        }
                    },
                    "id": req.id
                });
                write_response_shared(&stdout, &resp).await;
            }

            "notifications/initialized" => continue,

            "tools/list" => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": { "tools": tool_definitions() },
                    "id": req.id
                });
                write_response_shared(&stdout, &resp).await;
            }

            "tools/call" => {
                // Spawn tool call concurrently — does not block the stdin reader
                let sock = Arc::clone(&sock_path);
                let sid = Arc::clone(&session_id);
                let out = Arc::clone(&stdout);
                let id = req.id.clone();
                let params = req.params;
                tokio::spawn(async move {
                    let result =
                        handle_tool_call(&sock, params, sid.as_deref()).await;
                    let response = match result {
                        Ok(content) => serde_json::json!({
                            "jsonrpc": "2.0",
                            "result": { "content": content },
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
                    };
                    write_response_shared(&out, &response).await;
                });
            }

            other => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32601, "message": format!("Method not found: {}", other) },
                    "id": req.id
                });
                write_response_shared(&stdout, &resp).await;
            }
        }
    }

    Ok(())
}

/// Write a JSON-RPC response to a shared stdout (tokio Mutex-protected).
/// The entire write (json + newline + flush) is atomic w.r.t. the lock so
/// concurrent tasks never interleave their output.
async fn write_response_shared(
    stdout: &Arc<tokio::sync::Mutex<tokio::io::Stdout>>,
    resp: &serde_json::Value,
) {
    let json = match serde_json::to_string(resp) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("MCP neovim bridge: failed to serialize response: {}", e);
            return;
        }
    };
    let mut out = stdout.lock().await;
    if let Err(e) = out.write_all(json.as_bytes()).await {
        eprintln!("MCP neovim bridge: stdout write error: {}", e);
        return;
    }
    if let Err(e) = out.write_all(b"\n").await {
        eprintln!("MCP neovim bridge: stdout write error: {}", e);
        return;
    }
    if let Err(e) = out.flush().await {
        eprintln!("MCP neovim bridge: stdout flush error: {}", e);
    }
}
