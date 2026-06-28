/// MCP kanban server — runs as `opman mcp-kanban`.
///
/// Exposes tools that let a launched agent read its Kanban task and report
/// progress by moving lanes / adding notes. It is backend-agnostic: the task
/// id is supplied as a tool argument (seeded into the agent's brief), and the
/// web server's loopback URL + shared token are read from
/// `~/.config/opman/internal.json`. Speaks JSON-RPC 2.0 over stdio.
mod tools;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
    id: serde_json::Value,
}

/// Resolved internal API descriptor (`~/.config/opman/internal.json`).
#[derive(Clone)]
pub(crate) struct Internal {
    pub url: String,
    pub token: String,
    pub client: reqwest::Client,
}

fn load_internal() -> Option<Internal> {
    let path = dirs::config_dir()?.join("opman").join("internal.json");
    let content = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    Some(Internal {
        url: v.get("url")?.as_str()?.to_string(),
        token: v.get("token")?.as_str()?.to_string(),
        client: reqwest::Client::new(),
    })
}

pub async fn run_mcp_kanban_bridge() -> anyhow::Result<()> {
    let internal = load_internal();
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let n = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(e) => {
                eprintln!("MCP kanban bridge stdin read error: {}", e);
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
                    "serverInfo": { "name": "opman-kanban", "version": "1.0.0" }
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
                let text = tools::dispatch_tool(internal.as_ref(), req.params).await;
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": { "content": [{ "type": "text", "text": text }] },
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

async fn write_response(stdout: &mut tokio::io::Stdout, resp: &serde_json::Value) {
    let json = match serde_json::to_string(resp) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("MCP kanban bridge: serialize error: {}", e);
            return;
        }
    };
    let _ = stdout.write_all(json.as_bytes()).await;
    let _ = stdout.write_all(b"\n").await;
    let _ = stdout.flush().await;
}

fn tool_definitions() -> serde_json::Value {
    let task_id = serde_json::json!({
        "type": "string",
        "description": "The Kanban task id (provided in your task brief, e.g. tsk_...)."
    });
    serde_json::json!([
        {
            "name": "kanban_get_task",
            "description": "Fetch the Kanban task you are working on: title, description, tags, current lane, and the lanes you are allowed to move to next.",
            "inputSchema": {
                "type": "object",
                "properties": { "task_id": task_id },
                "required": ["task_id"]
            }
        },
        {
            "name": "kanban_set_lane",
            "description": "Move the task to a new lane to reflect your current stage. Only transitions allowed by the board's graph are accepted. `lane` may be a lane id or its display name.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": task_id,
                    "lane": { "type": "string", "description": "Target lane id or name (e.g. \"Implementing\")." }
                },
                "required": ["task_id", "lane"]
            }
        },
        {
            "name": "kanban_add_note",
            "description": "Append a progress note to the task's activity timeline so the board shows live progress.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": task_id,
                    "body": { "type": "string", "description": "The progress note text." }
                },
                "required": ["task_id", "body"]
            }
        },
        {
            "name": "kanban_complete",
            "description": "Mark the task ready for human review: moves it to the board's terminal review lane and records a summary. Do not move the task past this point.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": task_id,
                    "summary": { "type": "string", "description": "A short summary of the work done." }
                },
                "required": ["task_id"]
            }
        }
    ])
}
