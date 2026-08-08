//! `opman mcp-skills` — the stdio MCP server that puts skills in front of every runner.
//!
//! Same skeleton as the other bridges (`src/mcp_time/mod.rs`): a read-line loop where
//! EOF ends the process and a parse error is answered rather than fatal. It also handles
//! `notifications/initialized`, which the HTTP handler this replaces did not — a real
//! MCP client sends that immediately after `initialize`, got `-32601`, and many abort
//! the handshake there.

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use super::store::SkillStore;
use super::tools::{self, AuthLookup, NoAuthInfo};

const PROTOCOL: &str = "2024-11-05";

#[derive(Debug, Deserialize)]
struct McpRequest {
    method: String,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    id: Value,
}

pub async fn run_mcp_skills_bridge() -> anyhow::Result<()> {
    let store = SkillStore::from_env();
    run_skills_over(
        store,
        Box::new(NoAuthInfo),
        tokio::io::stdin(),
        tokio::io::stdout(),
    )
    .await
}

/// Generic over the streams so the loop is testable without a process.
pub(crate) async fn run_skills_over<R, W>(
    mut store: SkillStore,
    auth: Box<dyn AuthLookup + Send>,
    reader: R,
    mut writer: W,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                // A read error must not kill the server: the runner would see a dead
                // MCP child and drop every skill.
                eprintln!("mcp-skills: read error: {error}");
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<McpRequest>(&line) {
            Ok(request) => route(&mut store, auth.as_ref(), request),
            Err(_) => Some(json!({
                "jsonrpc": "2.0",
                "error": { "code": -32700, "message": "Parse error" },
                "id": Value::Null,
            })),
        };
        if let Some(response) = response {
            write_response(&mut writer, &response).await;
        }
    }
    Ok(())
}

fn route(store: &mut SkillStore, auth: &dyn AuthLookup, request: McpRequest) -> Option<Value> {
    let id = request.id;
    match request.method.as_str() {
        "initialize" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": PROTOCOL,
                // listChanged, because skills are edited while a session is live.
                "capabilities": { "tools": { "listChanged": true }, "prompts": { "listChanged": true } },
                "serverInfo": { "name": "opman-skills", "version": env!("CARGO_PKG_VERSION") },
            }
        })),
        // A notification takes no reply at all.
        "notifications/initialized" => None,
        "tools/list" => {
            store.refresh();
            Some(
                json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tools::tool_definitions(store, auth) } }),
            )
        }
        "tools/call" => {
            store.refresh();
            let result = tools::dispatch_tool(store, auth, request.params.as_ref());
            Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
        }
        // Prompts are user-initiated by spec, so they cannot drive auto-selection — but
        // in Claude Code they surface as `/mcp__opman-skills__<name>`, the closest legal
        // analogue to a native slash command without writing into the runner's own
        // skills directory.
        "prompts/list" => {
            store.refresh();
            Some(json!({ "jsonrpc": "2.0", "id": id, "result": { "prompts": prompts(store) } }))
        }
        "prompts/get" => {
            store.refresh();
            Some(prompt_get(store, request.params.as_ref(), id))
        }
        other => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {other}") },
        })),
    }
}

fn prompts(store: &SkillStore) -> Value {
    Value::Array(
        store
            .skills()
            .values()
            .map(|skill| json!({ "name": skill.name, "description": skill.description }))
            .collect(),
    )
}

fn prompt_get(store: &SkillStore, params: Option<&Value>, id: Value) -> Value {
    let name = params
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(skill) = store.get(name) else {
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": format!("Unknown prompt: {name}") },
        });
    };
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "description": skill.description,
            "messages": [{
                "role": "user",
                "content": { "type": "text", "text": skill.content },
            }],
        }
    })
}

async fn write_response<W: AsyncWrite + Unpin>(writer: &mut W, value: &Value) {
    let Ok(mut line) = serde_json::to_vec(value) else {
        return;
    };
    line.push(b'\n');
    let _ = writer.write_all(&line).await;
    let _ = writer.flush().await;
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod bridge_tests;
