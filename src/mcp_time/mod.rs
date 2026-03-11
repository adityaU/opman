/// MCP time server — runs as `opman --mcp-time`
///
/// Exposes three tools to the AI:
///   - `time_now`       — current time in the system timezone (or a given zone)
///   - `time_convert`   — convert a datetime from one timezone to another
///   - `time_zones`     — list/search IANA timezone names
///
/// The server speaks JSON-RPC 2.0 over stdin/stdout (standard MCP stdio transport).
/// No Unix socket is needed — all operations are pure computation.

mod tools;

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

pub async fn run_mcp_time_bridge() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let n = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(e) => {
                eprintln!("MCP time bridge stdin read error: {}", e);
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
                    "serverInfo": {
                        "name": "opman-time",
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

/// Write a JSON-RPC response to stdout. Swallows write errors so the bridge
/// never dies due to a transient stdout issue.
async fn write_response(stdout: &mut tokio::io::Stdout, resp: &serde_json::Value) {
    let json = match serde_json::to_string(resp) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("MCP time bridge: failed to serialize response: {}", e);
            return;
        }
    };
    if let Err(e) = stdout.write_all(json.as_bytes()).await {
        eprintln!("MCP time bridge: stdout write error: {}", e);
        return;
    }
    if let Err(e) = stdout.write_all(b"\n").await {
        eprintln!("MCP time bridge: stdout write error: {}", e);
        return;
    }
    if let Err(e) = stdout.flush().await {
        eprintln!("MCP time bridge: stdout flush error: {}", e);
    }
}

// ─── Tool definitions ────────────────────────────────────────────────────────

fn tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "time_now",
            "description": "Get the current date and time. Returns the time in the system's local timezone by default. Optionally specify an IANA timezone name (e.g. \"America/New_York\", \"Europe/London\", \"Asia/Tokyo\") to get the current time in that zone. Also always reports the system's default timezone.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "timezone": {
                        "type": "string",
                        "description": "Optional IANA timezone name (e.g. \"America/New_York\", \"UTC\", \"Asia/Kolkata\"). Defaults to the system local timezone."
                    }
                }
            }
        },
        {
            "name": "time_convert",
            "description": "Convert a date/time from one timezone to another. Accepts a datetime string and source/target IANA timezone names.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "datetime": {
                        "type": "string",
                        "description": "The datetime to convert. Accepted formats: \"2024-01-15 14:30:00\", \"2024-01-15T14:30:00\", \"2024-01-15 14:30\" (seconds optional). Time is interpreted in the `from_timezone`."
                    },
                    "from_timezone": {
                        "type": "string",
                        "description": "IANA source timezone name (e.g. \"America/Los_Angeles\"). Use \"local\" for the system timezone."
                    },
                    "to_timezone": {
                        "type": "string",
                        "description": "IANA target timezone name (e.g. \"Asia/Kolkata\"). Use \"local\" for the system timezone."
                    }
                },
                "required": ["datetime", "from_timezone", "to_timezone"]
            }
        },
        {
            "name": "time_zones",
            "description": "List or search IANA timezone names. Returns all timezones or filters by a search term. Useful for discovering valid timezone strings to pass to other time tools.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "search": {
                        "type": "string",
                        "description": "Optional case-insensitive search term to filter timezone names (e.g. \"india\", \"pacific\", \"europe\")."
                    }
                }
            }
        }
    ])
}

// ─── Tool dispatch ───────────────────────────────────────────────────────────

fn dispatch_tool(params: Option<serde_json::Value>) -> serde_json::Value {
    let params = params.unwrap_or(serde_json::json!({}));
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let text = match tool_name {
        "time_now" => tools::tool_time_now(&args),
        "time_convert" => tools::tool_time_convert(&args),
        "time_zones" => tools::tool_time_zones(&args),
        other => format!("Unknown tool: {}", other),
    };

    serde_json::json!([{ "type": "text", "text": text }])
}
