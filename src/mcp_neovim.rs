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

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp::{self, SocketRequest, SocketResponse};

// ─── JSON-RPC types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
    id: serde_json::Value,
}

// ─── Entry point ─────────────────────────────────────────────────────────────

pub async fn run_mcp_neovim_bridge(project_path: PathBuf) -> anyhow::Result<()> {
    let sock_path = mcp::socket_path_for_project(&project_path);
    // Read session ID from env var set by opencode PTY spawn, so all
    // socket requests route to the correct per-session resources.
    let session_id = std::env::var("OPENCODE_SESSION_ID").ok();
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            break; // EOF
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
                write_response(&mut stdout, &resp).await?;
                continue;
            }
        };

        let resp = match req.method.as_str() {
            "initialize" => serde_json::json!({
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
            }),

            "notifications/initialized" => continue,

            "tools/list" => serde_json::json!({
                "jsonrpc": "2.0",
                "result": { "tools": tool_definitions() },
                "id": req.id
            }),

            "tools/call" => {
                let result = handle_tool_call(&sock_path, req.params, session_id.as_deref()).await;
                match result {
                    Ok(content) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "result": { "content": content },
                        "id": req.id
                    }),
                    Err(e) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "result": {
                            "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                            "isError": true
                        },
                        "id": req.id
                    }),
                }
            }

            other => serde_json::json!({
                "jsonrpc": "2.0",
                "error": { "code": -32601, "message": format!("Method not found: {}", other) },
                "id": req.id
            }),
        };

        write_response(&mut stdout, &resp).await?;
    }

    Ok(())
}

async fn write_response(
    stdout: &mut tokio::io::Stdout,
    resp: &serde_json::Value,
) -> anyhow::Result<()> {
    stdout
        .write_all(serde_json::to_string(resp)?.as_bytes())
        .await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

// ─── Tool definitions ────────────────────────────────────────────────────────

fn tool_definitions() -> serde_json::Value {
    serde_json::json!([
        // ── File & Buffer ────────────────────────────────────────────
        {
            "name": "neovim_open",
            "description": "Open a file in the embedded Neovim editor. Optionally jump to a specific line number. The file will be displayed in the Neovim pane of the opman TUI.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to open (absolute or relative to the project root)."
                    },
                    "line": {
                        "type": "number",
                        "description": "Optional line number to jump to (1-indexed). The view will be centered on this line."
                    }
                },
                "required": ["file_path"]
            }
        },
        {
            "name": "neovim_read",
            "description": "Read lines from a buffer in the embedded Neovim editor. Returns the text content of the specified line range with line numbers. If file_path is provided, reads from that file's buffer; otherwise reads from the current buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to read. If omitted, reads the current buffer."
                    },
                    "start_line": {
                        "type": "number",
                        "description": "Start line (1-indexed, inclusive). Defaults to 1."
                    },
                    "end_line": {
                        "type": "number",
                        "description": "End line (1-indexed, inclusive). Defaults to the last line of the buffer. Use -1 for the last line."
                    }
                }
            }
        },
        {
            "name": "neovim_command",
            "description": "Execute a Vim ex-command in the embedded Neovim editor. For example: \"set number\", \"w\", \"buffers\", \"%s/foo/bar/g\", etc. Do not include the leading colon.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The Vim ex-command to execute (without the leading colon)."
                    }
                },
                "required": ["command"]
            }
        },
        {
            "name": "neovim_buffers",
            "description": "List all loaded buffers in the embedded Neovim editor. Returns buffer IDs and their associated file paths.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "neovim_info",
            "description": "Get information about the current state of the embedded Neovim editor: current buffer file path, cursor position (line, column), and total line count.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "neovim_write",
            "description": "Save a buffer (or all buffers) in the embedded Neovim editor. If file_path is provided, saves that file's buffer; otherwise saves the current buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to save. If omitted, saves the current buffer."
                    },
                    "all": {
                        "type": "boolean",
                        "description": "If true, save all modified buffers. If false or omitted, save only the targeted buffer."
                    }
                }
            }
        },
        {
            "name": "neovim_diff",
            "description": "Show unsaved changes in a Neovim buffer as a unified diff. Compares the buffer content against the file on disk. If file_path is provided, diffs that file's buffer; otherwise diffs the current buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to diff. If omitted, diffs the current buffer."
                    }
                }
            }
        },
        // ── LSP ──────────────────────────────────────────────────────
        {
            "name": "neovim_diagnostics",
            "description": "Get LSP diagnostics (errors, warnings, hints) from Neovim. Returns structured diagnostic info including file, line, severity, message, and source. Requires an LSP server to be attached to the buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to get diagnostics for. If omitted, uses the current buffer."
                    },
                    "buf_only": {
                        "type": "boolean",
                        "description": "If true, return diagnostics only for the targeted buffer. If false or omitted, return diagnostics for all open buffers (project-wide)."
                    }
                }
            }
        },
        {
            "name": "neovim_definition",
            "description": "Go to the definition of the symbol at the specified position using the LSP. Jumps to the definition and returns the location(s). Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file containing the symbol. If omitted, uses the current buffer."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the symbol. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the symbol. Defaults to current cursor column."
                    }
                }
            }
        },
        {
            "name": "neovim_references",
            "description": "Find all references to the symbol at the specified position using the LSP. Returns file paths, line numbers, and context for each reference. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file containing the symbol. If omitted, uses the current buffer."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the symbol. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the symbol. Defaults to current cursor column."
                    }
                }
            }
        },
        {
            "name": "neovim_hover",
            "description": "Get hover/type information for the symbol at the specified position from the LSP. Returns type signatures, documentation, etc. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file containing the symbol. If omitted, uses the current buffer."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the symbol. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the symbol. Defaults to current cursor column."
                    }
                }
            }
        },
        {
            "name": "neovim_symbols",
            "description": "Search for symbols using the LSP. Can search within a specific document or across the entire workspace. Returns symbol names, kinds, file locations, and line numbers. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to search for document symbols. If omitted, uses the current buffer. Ignored for workspace searches."
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to filter symbols. For workspace search, this filters by name. For document symbols, all symbols are returned (query is ignored)."
                    },
                    "workspace": {
                        "type": "boolean",
                        "description": "If true, search across the entire workspace. If false or omitted, search only the targeted document."
                    }
                }
            }
        },
        {
            "name": "neovim_code_actions",
            "description": "List available LSP code actions at the current cursor position. Code actions include quick-fixes, refactors, and source actions. Returns action titles and kinds. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to get code actions for. If omitted, uses the current buffer."
                    }
                }
            }
        },
        // ── Dev Flow ─────────────────────────────────────────────────
        {
            "name": "neovim_eval",
            "description": "Execute arbitrary Lua code inside the embedded Neovim instance and return the result. This is a powerful escape hatch for any Neovim operation not covered by the other tools. The code should use `return` to produce output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "Lua code to execute in Neovim. Use `return` to produce output. Has full access to `vim.*` APIs."
                    }
                },
                "required": ["code"]
            }
        },
        {
            "name": "neovim_grep",
            "description": "Search across project files using Neovim's vimgrep. Returns matching file paths, line numbers, and matched text. Useful for finding usages, patterns, or text across the codebase.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The search pattern (Vim regex syntax)."
                    },
                    "glob": {
                        "type": "string",
                        "description": "File glob pattern to limit the search scope. Defaults to \"**/*\". Examples: \"**/*.rs\", \"src/**/*.ts\", \"*.py\"."
                    }
                },
                "required": ["pattern"]
            }
        },
        // ── Editing ─────────────────────────────────────────────
        {
            "name": "neovim_edit",
            "description": "Replace a range of lines in a Neovim buffer with new text. This is the primary way to modify file content. Lines are 1-indexed and inclusive. If file_path is provided, edits that file's buffer; otherwise edits the current buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to edit. If omitted, edits the current buffer."
                    },
                    "start_line": {
                        "type": "number",
                        "description": "First line to replace (1-indexed, inclusive)."
                    },
                    "end_line": {
                        "type": "number",
                        "description": "Last line to replace (1-indexed, inclusive)."
                    },
                    "new_text": {
                        "type": "string",
                        "description": "The replacement text. Use newlines (\\n) to separate multiple lines. Pass an empty string to delete the specified lines."
                    }
                },
                "required": ["start_line", "end_line", "new_text"]
            }
        },
        {
            "name": "neovim_undo",
            "description": "Undo or redo changes in a Neovim buffer. Positive count undoes that many changes, negative count redoes. If file_path is provided, operates on that file's buffer; otherwise on the current buffer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to undo in. If omitted, uses the current buffer."
                    },
                    "count": {
                        "type": "number",
                        "description": "Number of changes to undo (positive) or redo (negative). Defaults to 1 undo."
                    }
                }
            }
        },
        // ── LSP Refactoring ─────────────────────────────────────
        {
            "name": "neovim_rename",
            "description": "Rename a symbol across the project using the LSP. If file_path is provided, uses that file's buffer context; otherwise uses the current buffer. Requires an LSP server with rename support.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file containing the symbol. If omitted, uses the current buffer."
                    },
                    "new_name": {
                        "type": "string",
                        "description": "The new name for the symbol."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the symbol. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the symbol. Defaults to current cursor column."
                    }
                },
                "required": ["new_name"]
            }
        },
        {
            "name": "neovim_format",
            "description": "Format a buffer using the LSP formatter (e.g., rustfmt, prettier, black). If file_path is provided, formats that file's buffer; otherwise formats the current buffer. Requires an LSP server with formatting support.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file to format. If omitted, formats the current buffer."
                    }
                }
            }
        },
        {
            "name": "neovim_signature",
            "description": "Get function signature help at the specified position from the LSP. Shows parameter names, types, and documentation for function calls. If file_path is provided, uses that file's buffer context; otherwise uses the current buffer. Requires an LSP server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path of the file. If omitted, uses the current buffer."
                    },
                    "line": {
                        "type": "number",
                        "description": "Line number (1-indexed) of the call site. Defaults to current cursor line."
                    },
                    "col": {
                        "type": "number",
                        "description": "Column number (0-indexed) of the call site. Defaults to current cursor column."
                    }
                }
            }
        }
    ])
}

// ─── Tool dispatch ───────────────────────────────────────────────────────────

async fn handle_tool_call(
    sock_path: &std::path::Path,
    params: Option<serde_json::Value>,
    session_id: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let params = params.unwrap_or(serde_json::json!({}));
    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let socket_req = match tool_name {
        // ── File & Buffer ────────────────────────────────────────
        "neovim_open" => {
            let file_path = arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("neovim_open requires 'file_path' argument"))?;
            SocketRequest {
                op: "nvim_open".into(),
                file_path: Some(file_path.to_string()),
                line: arguments.get("line").and_then(|v| v.as_i64()),
                ..Default::default()
            }
        }
        "neovim_read" => SocketRequest {
            op: "nvim_read".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            line: arguments.get("start_line").and_then(|v| v.as_i64()),
            end_line: arguments.get("end_line").and_then(|v| v.as_i64()),
            ..Default::default()
        },
        "neovim_command" => {
            let command = arguments
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("neovim_command requires 'command' argument"))?;
            SocketRequest {
                op: "nvim_command".into(),
                command: Some(command.to_string()),
                ..Default::default()
            }
        }
        "neovim_buffers" => SocketRequest {
            op: "nvim_buffers".into(),
            ..Default::default()
        },
        "neovim_info" => SocketRequest {
            op: "nvim_info".into(),
            ..Default::default()
        },
        "neovim_write" => SocketRequest {
            op: "nvim_write".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            all: arguments.get("all").and_then(|v| v.as_bool()),
            ..Default::default()
        },
        "neovim_diff" => SocketRequest {
            op: "nvim_diff".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            ..Default::default()
        },
        // ── LSP ──────────────────────────────────────────────────
        "neovim_diagnostics" => SocketRequest {
            op: "nvim_diagnostics".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            buf_only: arguments.get("buf_only").and_then(|v| v.as_bool()),
            ..Default::default()
        },
        "neovim_definition" => SocketRequest {
            op: "nvim_definition".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            line: arguments.get("line").and_then(|v| v.as_i64()),
            col: arguments.get("col").and_then(|v| v.as_i64()),
            ..Default::default()
        },
        "neovim_references" => SocketRequest {
            op: "nvim_references".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            line: arguments.get("line").and_then(|v| v.as_i64()),
            col: arguments.get("col").and_then(|v| v.as_i64()),
            ..Default::default()
        },
        "neovim_hover" => SocketRequest {
            op: "nvim_hover".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            line: arguments.get("line").and_then(|v| v.as_i64()),
            col: arguments.get("col").and_then(|v| v.as_i64()),
            ..Default::default()
        },
        "neovim_symbols" => SocketRequest {
            op: "nvim_symbols".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            query: arguments
                .get("query")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            workspace: arguments.get("workspace").and_then(|v| v.as_bool()),
            ..Default::default()
        },
        "neovim_code_actions" => SocketRequest {
            op: "nvim_code_actions".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            ..Default::default()
        },
        // ── Dev Flow ─────────────────────────────────────────────
        "neovim_eval" => {
            let code = arguments
                .get("code")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("neovim_eval requires 'code' argument"))?;
            SocketRequest {
                op: "nvim_eval".into(),
                command: Some(code.to_string()),
                ..Default::default()
            }
        }
        "neovim_grep" => {
            let pattern = arguments
                .get("pattern")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("neovim_grep requires 'pattern' argument"))?;
            SocketRequest {
                op: "nvim_grep".into(),
                query: Some(pattern.to_string()),
                glob: arguments
                    .get("glob")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                ..Default::default()
            }
        }
        // ── Editing ─────────────────────────────────────────
        "neovim_edit" => {
            let start_line = arguments
                .get("start_line")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("neovim_edit requires 'start_line' argument"))?;
            let end_line = arguments
                .get("end_line")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("neovim_edit requires 'end_line' argument"))?;
            let new_text = arguments
                .get("new_text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("neovim_edit requires 'new_text' argument"))?;
            SocketRequest {
                op: "nvim_edit".into(),
                file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
                line: Some(start_line),
                end_line: Some(end_line),
                new_text: Some(new_text.to_string()),
                ..Default::default()
            }
        }
        "neovim_undo" => SocketRequest {
            op: "nvim_undo".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            count: arguments.get("count").and_then(|v| v.as_i64()),
            ..Default::default()
        },
        // ── LSP Refactoring ─────────────────────────────────
        "neovim_rename" => {
            let new_name = arguments
                .get("new_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("neovim_rename requires 'new_name' argument"))?;
            SocketRequest {
                op: "nvim_rename".into(),
                file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
                new_name: Some(new_name.to_string()),
                line: arguments.get("line").and_then(|v| v.as_i64()),
                col: arguments.get("col").and_then(|v| v.as_i64()),
                ..Default::default()
            }
        }
        "neovim_format" => SocketRequest {
            op: "nvim_format".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            ..Default::default()
        },
        "neovim_signature" => SocketRequest {
            op: "nvim_signature".into(),
            file_path: arguments.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            line: arguments.get("line").and_then(|v| v.as_i64()),
            col: arguments.get("col").and_then(|v| v.as_i64()),
            ..Default::default()
        },
        other => {
            return Ok(serde_json::json!([{
                "type": "text",
                "text": format!("Unknown tool: {}", other)
            }]));
        }
    };

    // Inject session_id into the request for correct routing
    let mut socket_req = socket_req;
    socket_req.session_id = session_id.map(|s| s.to_string());

    let resp = send_socket_request(sock_path, &socket_req).await?;
    format_response(tool_name, &resp)
}

/// Send a SocketRequest over the Unix socket and return the response.
async fn send_socket_request(
    sock_path: &std::path::Path,
    request: &SocketRequest,
) -> anyhow::Result<SocketResponse> {
    use tokio::io::AsyncReadExt;
    use tokio::net::UnixStream;

    let mut stream = UnixStream::connect(sock_path).await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to connect to manager socket at {:?}: {}. Is opman running?",
            sock_path,
            e
        )
    })?;

    let req_json = serde_json::to_string(request)?;
    stream.write_all(req_json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    // Shutdown write side so the server knows we are done sending
    stream.shutdown().await?;

    // Read response
    let mut resp_buf = Vec::new();
    stream.read_to_end(&mut resp_buf).await?;
    let resp_str = String::from_utf8_lossy(&resp_buf);

    serde_json::from_str(resp_str.trim())
        .map_err(|e| anyhow::anyhow!("Invalid response from manager: {}", e))
}

/// Map file extension to markdown language identifier for syntax highlighting.
pub fn ext_to_lang(path: &str) -> &str {
    match path.rsplit('.').next().unwrap_or("") {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        "rb" => "ruby",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "cs" => "csharp",
        "lua" => "lua",
        "sh" | "bash" | "zsh" => "bash",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "sql" => "sql",
        "md" | "markdown" => "markdown",
        "zig" => "zig",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "r" | "R" => "r",
        "dart" => "dart",
        "vim" => "vim",
        "el" | "lisp" | "cl" => "lisp",
        "clj" | "cljs" => "clojure",
        "php" => "php",
        "pl" | "pm" => "perl",
        "scala" | "sc" => "scala",
        _ => "",
    }
}

/// Try to pretty-print a JSON string. Returns the original if it fails.
fn try_pretty_json(s: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| s.to_string()),
        Err(_) => s.to_string(),
    }
}

/// Convert a SocketResponse to MCP content format with proper markdown formatting.
///
/// Wraps output in markdown code fences with appropriate language identifiers
/// so that the AI client can render syntax-highlighted content.
fn format_response(tool_name: &str, resp: &SocketResponse) -> anyhow::Result<serde_json::Value> {
    if !resp.ok {
        let error_msg = resp.error.as_deref().unwrap_or("Unknown error");
        return Ok(serde_json::json!([{
            "type": "text",
            "text": format!("Error: {}", error_msg)
        }]));
    }

    let text = resp.output.as_deref().unwrap_or("OK");

    let formatted = match tool_name {
        // ── File content: already wrapped in code fence by app.rs ───
        "neovim_read" => text.to_string(),

        // ── Diff output ──────────────────────────────────────────────
        "neovim_diff" => {
            if text.contains("No unsaved") || !text.contains("@@") {
                text.to_string()
            } else {
                format!("```diff\n{}\n```", text)
            }
        }

        // ── LSP hover: already markdown from LSP, pass through ──────
        "neovim_hover" => text.to_string(),

        // ── JSON responses: pretty-print and wrap ────────────────────
        "neovim_diagnostics"
        | "neovim_definition"
        | "neovim_references"
        | "neovim_symbols"
        | "neovim_code_actions"
        | "neovim_grep"
        | "neovim_rename"
        | "neovim_format"
        | "neovim_signature" => {
            let pretty = try_pretty_json(text);
            format!("```json\n{}\n```", pretty)
        }

        // ── Lua eval: wrap in code fence ────────────────────────────
        "neovim_eval" => format!("```\n{}\n```", text),

        // ── Plain text for everything else ──────────────────────────
        _ => text.to_string(),
    };

    Ok(serde_json::json!([{
        "type": "text",
        "text": formatted
    }]))
}
