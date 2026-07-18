/// MCP kanban server — runs as `opman mcp-kanban`.
///
/// Exposes tools that let a launched agent read its Kanban task and report
/// progress by moving lanes / adding notes. It is backend-agnostic: the task
/// id is supplied as a tool argument (seeded into the agent's brief), and the
/// web server's loopback URL + shared token are read from
/// `~/.config/opman/internal.json`. Speaks JSON-RPC 2.0 over stdio.
mod tools;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

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
    load_internal_from(&path)
}

/// Parse an `internal.json` descriptor from a specific path. Extracted so the
/// parsing/validation logic is testable without depending on the real config
/// directory. Returns `None` if the file is missing, malformed, or lacks the
/// required `url`/`token` string fields.
pub(crate) fn load_internal_from(path: &std::path::Path) -> Option<Internal> {
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
    run_kanban_bridge(internal, tokio::io::stdin(), tokio::io::stdout()).await
}

/// Generic stdio read-loop, parameterized over reader/writer for testability.
async fn run_kanban_bridge<R, W>(
    internal: Option<Internal>,
    reader: R,
    mut writer: W,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(reader);
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
                write_response(&mut writer, &resp).await;
                continue;
            }
        };

        if let Some(resp) = route_request(internal.as_ref(), &req.method, req.params, req.id).await {
            write_response(&mut writer, &resp).await;
        }
    }
    Ok(())
}

/// Route a JSON-RPC request to its response. Returns `None` for notifications
/// that require no reply.
async fn route_request(
    internal: Option<&Internal>,
    method: &str,
    params: Option<serde_json::Value>,
    id: serde_json::Value,
) -> Option<serde_json::Value> {
    let resp = match method {
        "initialize" => serde_json::json!({
            "jsonrpc": "2.0",
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "opman-kanban", "version": "1.0.0" }
            },
            "id": id
        }),
        "notifications/initialized" => return None,
        "tools/list" => serde_json::json!({
            "jsonrpc": "2.0",
            "result": { "tools": tool_definitions() },
            "id": id
        }),
        "tools/call" => {
            let text = tools::dispatch_tool(internal, params).await;
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": { "content": [{ "type": "text", "text": text }] },
                "id": id
            })
        }
        other => serde_json::json!({
            "jsonrpc": "2.0",
            "error": { "code": -32601, "message": format!("Method not found: {}", other) },
            "id": id
        }),
    };
    Some(resp)
}

async fn write_response<W: AsyncWrite + Unpin>(stdout: &mut W, resp: &serde_json::Value) {
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
        },
        {
            "name": "kanban_list_tasks",
            "description": "List/search other tasks on your board. Use task_id to anchor to your board, then filter: by lane (all tasks in a lane), by tags (match any), and/or by a free-text query (matched against title, description and tags). Archived tasks are excluded unless include_archived is set. Returns compact task summaries (id, title, lane, tags, priority, run_state).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": task_id,
                    "lane": { "type": "string", "description": "Only tasks in this lane (id or display name)." },
                    "tags": { "type": "array", "items": { "type": "string" }, "description": "Match tasks having ANY of these tags (case-insensitive)." },
                    "query": { "type": "string", "description": "Free-text search over title, description and tags (case-insensitive)." },
                    "include_archived": { "type": "boolean", "description": "Include archived tasks (default false)." }
                },
                "required": ["task_id"]
            }
        },
        {
            "name": "kanban_board_summary",
            "description": "Get an overview of your board: every lane with its active/archived task counts, WIP limit, terminal flag, and the lanes each lane may move to. Use task_id to anchor to your board. Good for orienting before drilling into a lane with kanban_list_tasks.",
            "inputSchema": {
                "type": "object",
                "properties": { "task_id": task_id },
                "required": ["task_id"]
            }
        },
        {
            "name": "kanban_read_notes",
            "description": "Read the activity-timeline notes for one or more tasks (e.g. to learn what other tasks decided or where they got stuck). Pass task_ids; an empty list defaults to your own task. task_id anchors to your board and tasks on other boards are skipped. Returns each task's notes (author, body, lane transitions, timestamp).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": task_id,
                    "task_ids": { "type": "array", "items": { "type": "string" }, "description": "Task ids whose notes to read (e.g. tsk_...). Empty = your own task." }
                },
                "required": ["task_id"]
            }
        }
    ])
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;

#[cfg(test)]
#[path = "mod_loop_tests.rs"]
mod mod_loop_tests;

#[cfg(test)]
#[path = "dispatch_route_tests.rs"]
mod dispatch_route_tests;
