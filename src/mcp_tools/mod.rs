//! `opman mcp-tools`: common coding tools as a stdio MCP server.
//!
//! Tools: `bash`, `read_file`, `write_file`, `edit_file`, `list_dir`, `glob`, `grep` and
//! `web_fetch`. Each has a JSON input schema and an output schema, and returns both a
//! readable text block and `structuredContent`, so agents and code-mode runners can use
//! them the same way.
//!
//! Paths: relative paths start at the project folder given on the command line. Absolute
//! paths are allowed anywhere, as with the agents' native tools. This is not a sandbox.
//!
//! Calls run concurrently. `notifications/cancelled`, or the client closing stdin, aborts
//! the call; dropping a running `bash` kills its whole process group.

mod atomic;
mod bash;
mod call;
mod capture;
mod catalog;
mod catalog_files;
mod catalog_search;
mod edit_file;
mod glob;
mod grep;
mod grep_scan;
mod html;
mod limits;
mod lines;
mod list_dir;
mod output;
mod process;
mod read_file;
mod rpc;
mod schema;
mod walk;
mod web_fetch;
mod workspace;
mod write_file;

use rpc::Incoming;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use workspace::Workspace;

pub async fn run_mcp_tools_bridge(project_path: PathBuf) -> anyhow::Result<()> {
    serve(
        tokio::io::stdin(),
        tokio::io::stdout(),
        Workspace::new(project_path),
    )
    .await
}

/// The read loop, generic over the streams so tests can drive it in memory.
async fn serve<R, W>(reader: R, writer: W, workspace: Workspace) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (out, inbox) = mpsc::channel::<Value>(64);
    let writer_task = tokio::spawn(write_replies(writer, inbox));
    let workspace = std::sync::Arc::new(workspace);
    let mut running: HashMap<String, AbortHandle> = HashMap::new();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let message: Incoming = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(e) => {
                let _ = out.send(rpc::parse_error(&e.to_string())).await;
                continue;
            }
        };
        match (message.method.as_str(), message.id) {
            ("notifications/cancelled", None) => {
                let id = message.params.get("requestId").map(Value::to_string);
                if let Some(handle) = id.and_then(|id| running.remove(&id)) {
                    handle.abort();
                }
            }
            (_, None) => {} // other notifications, including `initialized`, get no reply
            ("tools/call", Some(id)) => {
                running.retain(|_, handle| !handle.is_finished());
                let (workspace, out) = (workspace.clone(), out.clone());
                let task = tokio::spawn({
                    let id = id.clone();
                    async move {
                        let result = call::run(&workspace, message.params, &out).await;
                        let _ = out.send(rpc::result_reply(&id, result)).await;
                    }
                });
                running.insert(id.to_string(), task.abort_handle());
            }
            (method, Some(id)) => {
                let _ = out.send(simple_reply(method, &id, &message.params)).await;
            }
        }
    }
    // The client is gone: stop whatever is still running, which also kills its processes.
    for handle in running.values() {
        handle.abort();
    }
    drop(out);
    let _ = writer_task.await;
    Ok(())
}

fn simple_reply(method: &str, id: &Value, params: &Value) -> Value {
    match method {
        "initialize" => rpc::result_reply(id, rpc::initialize_result(params)),
        "ping" => rpc::result_reply(id, json!({})),
        "tools/list" => rpc::result_reply(id, json!({ "tools": catalog::definitions() })),
        other => rpc::error_reply(id, -32601, &format!("Method not found: {other}")),
    }
}

/// The only writer of stdout, so replies from concurrent calls never interleave.
async fn write_replies<W: AsyncWrite + Unpin>(mut writer: W, mut inbox: mpsc::Receiver<Value>) {
    while let Some(reply) = inbox.recv().await {
        let Ok(mut text) = serde_json::to_string(&reply) else {
            continue;
        };
        text.push('\n');
        if writer.write_all(text.as_bytes()).await.is_err() || writer.flush().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
#[path = "bash_tests.rs"]
mod bash_tests;
#[cfg(test)]
#[path = "capture_tests.rs"]
mod capture_tests;
#[cfg(test)]
#[path = "file_tools_tests.rs"]
mod file_tools_tests;
#[cfg(test)]
#[path = "html_tests.rs"]
mod html_tests;
#[cfg(test)]
#[path = "protocol_tests.rs"]
mod protocol_tests;
#[cfg(test)]
#[path = "search_tests.rs"]
mod search_tests;
#[cfg(test)]
#[path = "web_fetch_tests.rs"]
mod web_fetch_tests;
