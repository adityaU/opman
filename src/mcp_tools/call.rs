//! Running one `tools/call`, with keep-alive progress for long calls.

use super::catalog::Tool;
use super::output::{call_result, ToolError};
use super::rpc::progress;
use super::workspace::Workspace;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

/// OpenCode gives up on a tool that is silent for 60 s unless progress arrives.
const PROGRESS_EVERY: Duration = Duration::from_secs(15);

/// Runs the call named in `params` and returns the MCP result. While it runs, sends a
/// progress notification now and then if the client gave a `progressToken`.
pub async fn run(workspace: &Workspace, params: Value, out: &Sender<Value>) -> Value {
    let token = params.pointer("/_meta/progressToken").cloned();
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let Some(tool) = Tool::parse(name) else {
        return call_result(Err(ToolError::new(format!("Unknown tool: {name}"))));
    };
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    let work = tool.call(workspace, arguments);
    tokio::pin!(work);

    let mut ticker = tokio::time::interval(PROGRESS_EVERY);
    ticker.tick().await; // the first tick is immediate
    let mut step = 0;
    let result = loop {
        tokio::select! {
            result = &mut work => break result,
            _ = ticker.tick() => {
                let Some(token) = &token else { continue };
                step += 1;
                let _ = out.send(progress(token, step, "still running")).await;
            }
        }
    };
    call_result(result)
}
