//! MCP browser server — runs as `opman mcp-browser`.
//!
//! Gives a runner control of the browser panes in the user's workspace. Every call goes
//! to the web server's loopback `/internal/browser` endpoint, so the tab the agent drives
//! is literally the tab on screen — there is no second, invisible browser.
//!
//! Reads are the reason this exists. `browser_snapshot` answers with a `[ref=eN]` outline
//! of the actionable elements, not markup; a page that would cost tens of thousands of
//! tokens as HTML costs a few hundred, and stays that size as the page grows. See
//! [`crate::browser`].

mod tools;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

pub(crate) use crate::loopback::Loopback as Internal;

#[derive(Debug, Deserialize)]
struct McpRequest {
    method: String,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    id: Value,
}

/// The project this server was launched for. Sent with every call so an agent never has
/// to name — or even know — a pane id.
#[derive(Clone, Debug)]
pub struct Project(String);

impl Project {
    fn as_str(&self) -> &str {
        &self.0
    }
}

pub async fn run_mcp_browser_bridge(project_path: std::path::PathBuf) -> anyhow::Result<()> {
    let internal = Internal::load();
    // Canonicalised so the id derived from it matches the one the widget derives from the
    // project path the workspace holds; `.` and a symlinked checkout would otherwise be
    // two different browsers for one repo.
    let project = Project(
        std::fs::canonicalize(&project_path)
            .unwrap_or(project_path)
            .to_string_lossy()
            .into_owned(),
    );
    run_browser_bridge(internal, project, tokio::io::stdin(), tokio::io::stdout()).await
}

/// Generic stdio read loop, parameterized over reader/writer so the protocol is testable
/// without a process.
async fn run_browser_bridge<R, W>(
    internal: Option<Internal>,
    project: Project,
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
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("MCP browser bridge stdin read error: {e}");
                continue;
            }
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: McpRequest = match serde_json::from_str(trimmed) {
            Ok(request) => request,
            Err(e) => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32700, "message": format!("Parse error: {e}") },
                    "id": Value::Null,
                });
                write_response(&mut writer, &response).await;
                continue;
            }
        };

        if let Some(response) = route_request(
            internal.as_ref(),
            &project,
            &request.method,
            request.params,
            request.id,
        )
        .await
        {
            write_response(&mut writer, &response).await;
        }
    }
    Ok(())
}

/// Route a JSON-RPC request to its response. `None` for notifications, which take no
/// reply.
async fn route_request(
    internal: Option<&Internal>,
    project: &Project,
    method: &str,
    params: Option<Value>,
    id: Value,
) -> Option<Value> {
    let response = match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "opman-browser", "version": "1.0.0" },
            },
            "id": id,
        }),
        "notifications/initialized" => return None,
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "result": tools::definitions(),
            "id": id,
        }),
        "tools/call" => {
            let text = tools::dispatch_tool(internal, project, params).await;
            json!({
                "jsonrpc": "2.0",
                "result": { "content": [{ "type": "text", "text": text }] },
                "id": id,
            })
        }
        other => json!({
            "jsonrpc": "2.0",
            "error": { "code": -32601, "message": format!("Method not found: {other}") },
            "id": id,
        }),
    };
    Some(response)
}

/// POST one tagged operation to the loopback browser API.
pub(crate) async fn post(internal: &Internal, body: Value) -> Result<Value, String> {
    let response = internal
        .post("/internal/browser")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("{} {}", status.as_u16(), text.trim()));
    }
    serde_json::from_str(&text).map_err(|e| format!("malformed response: {e}"))
}

async fn write_response<W: AsyncWrite + Unpin>(writer: &mut W, response: &Value) {
    let Ok(json) = serde_json::to_string(response) else {
        eprintln!("MCP browser bridge: failed to serialize a response");
        return;
    };
    let _ = writer.write_all(json.as_bytes()).await;
    let _ = writer.write_all(b"\n").await;
    let _ = writer.flush().await;
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
