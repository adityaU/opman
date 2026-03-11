// ─── Tool dispatch: main entry + file/buffer/LSP arms ────────────────────────

use crate::mcp::SocketRequest;

use super::dispatch_edit::dispatch_edit_devflow_refactor;
use super::format::format_response;
use super::socket::send_socket_request;

pub(super) async fn handle_tool_call(
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

    let socket_req = match dispatch_tool(tool_name, &arguments)? {
        Some(req) => req,
        None => {
            return Ok(serde_json::json!([{
                "type": "text",
                "text": format!("Unknown tool: {}", tool_name)
            }]));
        }
    };

    // Inject session_id into the request for correct routing
    let mut socket_req = socket_req;
    socket_req.session_id = session_id.map(|s| s.to_string());

    let resp = send_socket_request(sock_path, &socket_req).await?;
    format_response(tool_name, &resp)
}

/// Map a tool name + arguments into a SocketRequest. Returns None for unknown tools.
fn dispatch_tool(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> anyhow::Result<Option<SocketRequest>> {
    let req = match tool_name {
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
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
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
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            all: arguments.get("all").and_then(|v| v.as_bool()),
            ..Default::default()
        },
        "neovim_diff" => SocketRequest {
            op: "nvim_diff".into(),
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ..Default::default()
        },
        // ── LSP ──────────────────────────────────────────────────
        "neovim_diagnostics" => SocketRequest {
            op: "nvim_diagnostics".into(),
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            buf_only: arguments.get("buf_only").and_then(|v| v.as_bool()),
            ..Default::default()
        },
        "neovim_definition" => SocketRequest {
            op: "nvim_definition".into(),
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            line: arguments.get("line").and_then(|v| v.as_i64()),
            col: arguments.get("col").and_then(|v| v.as_i64()),
            ..Default::default()
        },
        "neovim_references" => SocketRequest {
            op: "nvim_references".into(),
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            line: arguments.get("line").and_then(|v| v.as_i64()),
            col: arguments.get("col").and_then(|v| v.as_i64()),
            ..Default::default()
        },
        "neovim_hover" => SocketRequest {
            op: "nvim_hover".into(),
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            line: arguments.get("line").and_then(|v| v.as_i64()),
            col: arguments.get("col").and_then(|v| v.as_i64()),
            ..Default::default()
        },
        "neovim_symbols" => SocketRequest {
            op: "nvim_symbols".into(),
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            query: arguments
                .get("query")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            workspace: arguments.get("workspace").and_then(|v| v.as_bool()),
            ..Default::default()
        },
        "neovim_code_actions" => SocketRequest {
            op: "nvim_code_actions".into(),
            file_path: arguments
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ..Default::default()
        },
        // ── Dev Flow + Editing + LSP Refactoring ─────────────────
        "neovim_eval" | "neovim_grep" | "neovim_edit_and_save" | "neovim_undo"
        | "neovim_rename" | "neovim_format" | "neovim_signature" => {
            dispatch_edit_devflow_refactor(tool_name, arguments)?
        }
        _ => return Ok(None),
    };
    Ok(Some(req))
}
