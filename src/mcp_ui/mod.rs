/// MCP UI render server — runs as `opman mcp-ui`
///
/// Exposes one tool to the AI:
///   - `ui_render` — render rich UI blocks in the user's session timeline
///
/// Supports delta updates via `render_id` + `operation` fields, allowing
/// the agent to modify previously rendered UI (e.g., step-by-step progress).
///
/// The server speaks JSON-RPC 2.0 over stdin/stdout (standard MCP stdio transport).

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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

pub async fn run_mcp_ui_bridge() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let n = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(e) => {
                eprintln!("MCP UI bridge stdin read error: {}", e);
                continue;
            }
        };
        if n == 0 {
            break;
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
                write_response(&mut stdout, &resp).await;
                continue;
            }
        };

        let resp = match req.method.as_str() {
            "initialize" => serde_json::json!({
                "jsonrpc": "2.0",
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "opman-ui", "version": "1.1.0" }
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
                let result = dispatch_tool(req.params);
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": { "content": result },
                    "id": req.id
                })
            }

            other => serde_json::json!({
                "jsonrpc": "2.0",
                "error": { "code": -32601, "message": format!("Method not found: {}", other) },
                "id": req.id
            }),
        };

        write_response(&mut stdout, &resp).await;
    }

    Ok(())
}

/// Write a JSON-RPC response to stdout.
async fn write_response(stdout: &mut tokio::io::Stdout, resp: &serde_json::Value) {
    let json = match serde_json::to_string(resp) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("MCP UI bridge: failed to serialize response: {}", e);
            return;
        }
    };
    let _ = stdout.write_all(json.as_bytes()).await;
    let _ = stdout.write_all(b"\n").await;
    let _ = stdout.flush().await;
}

// ─── Tool definitions ────────────────────────────────────────────────────────

fn tool_definitions() -> serde_json::Value {
    serde_json::json!([{
        "name": "ui_render",
        "description": include_str!("tool_description.txt"),
        "inputSchema": {
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title displayed in the accordion header."
                },
                "blocks": {
                    "type": "array",
                    "description": "Array of UI blocks to render.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "description": "Block type: card, table, kv, status, progress, alert, button, form, markdown, steps, divider, code, metric, grid, flex, image, pdf, link, accordion, chart, tabs, callout, badge, blockquote, list, stat-group, diff, timeline, terminal, file-tree, avatar, tag-group, toggle, video, audio, separator"
                            },
                            "data": {
                                "type": "object",
                                "description": "Block-specific data. Shape depends on block type."
                            }
                        },
                        "required": ["type", "data"]
                    }
                },
                "render_id": {
                    "type": "string",
                    "description": "Optional stable ID for delta updates. When set, subsequent calls with the same render_id update the existing UI instead of creating a new one."
                },
                "operation": {
                    "type": "string",
                    "enum": ["replace", "append", "update"],
                    "description": "Delta operation (requires render_id). replace=overwrite all blocks, append=add blocks to end, update=patch blocks by index."
                }
            },
            "required": ["title", "blocks"]
        }
    }])
}

// ─── Tool dispatch ───────────────────────────────────────────────────────────

fn dispatch_tool(params: Option<serde_json::Value>) -> serde_json::Value {
    let params = params.unwrap_or(serde_json::json!({}));
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    match tool_name {
        "ui_render" => handle_ui_render(&args),
        other => serde_json::json!([{
            "type": "text",
            "text": format!("Unknown tool: {}", other)
        }]),
    }
}

/// Validate and echo the ui_render payload.
fn handle_ui_render(arguments: &serde_json::Value) -> serde_json::Value {
    let title = arguments
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("UI");

    let blocks = match arguments.get("blocks").and_then(|v| v.as_array()) {
        Some(b) if !b.is_empty() => b,
        _ => {
            return serde_json::json!([{
                "type": "text",
                "text": "ui_render requires a non-empty 'blocks' array"
            }]);
        }
    };

    for (i, block) in blocks.iter().enumerate() {
        if block.get("type").and_then(|v| v.as_str()).is_none() {
            return serde_json::json!([{
                "type": "text",
                "text": format!("Block {} missing 'type' field", i)
            }]);
        }
        if block.get("data").is_none() {
            return serde_json::json!([{
                "type": "text",
                "text": format!("Block {} missing 'data' field", i)
            }]);
        }
    }

    let render_id = arguments.get("render_id").and_then(|v| v.as_str());
    let operation = arguments.get("operation").and_then(|v| v.as_str());

    let desc = match (render_id, operation) {
        (Some(rid), Some(op)) => {
            format!("Rendered UI: {} ({} blocks, {}:{})", title, blocks.len(), op, rid)
        }
        _ => format!("Rendered UI: {} ({} blocks)", title, blocks.len()),
    };

    serde_json::json!([{ "type": "text", "text": desc }])
}
