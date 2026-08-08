//! MCP ask server — runs as `opman mcp-ask <dir>`.
//!
//! Exposes one tool, `ask_user_question`, which raises opman's question card and blocks
//! until the user answers.
//!
//! It exists because asking the user something is the one capability no harness exposes
//! the same way. ACP has no primitive for it — `session/request_permission` is an
//! allow/reject gate, not a choice — and Claude's ACP adapter disables its own
//! `AskUserQuestion` tool outright. MCP is the surface every harness behind every runner
//! already speaks, so routing the question through it makes the same card appear whatever
//! is actually running.
//!
//! Two things separate this from opman's other stdio servers. Calls are spawned rather
//! than awaited in the read loop, because a question outlives every other tool call and
//! blocking the pipe would stall the `notifications/cancelled` that ends it. And each call
//! ticks `notifications/progress` while it waits, without which clients kill it long
//! before the user has read the question.

mod progress;
mod tools;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tokio::task::{AbortHandle, JoinSet};

use crate::loopback::Loopback;

/// The session whose card this question belongs to. Absent for a runner whose MCP config
/// is process-wide, in which case the web server falls back to the newest session in the
/// project directory.
const SESSION_ENV: &str = "OPENCODE_SESSION_ID";

/// How long a closing pipe waits for already-answered calls to write their result.
const SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_millis(250);

#[derive(Debug, Deserialize)]
struct RpcRequest {
    method: String,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    id: Value,
}

/// Everything one call needs, resolved once at startup.
struct Context {
    loopback: Option<Loopback>,
    session: Option<String>,
    directory: String,
}

/// In-flight calls, so `notifications/cancelled` can end the one it names.
#[derive(Default)]
struct InFlight(HashMap<String, AbortHandle>);

impl InFlight {
    fn track(&mut self, id: &Value, handle: AbortHandle) {
        self.0.insert(key(id), handle);
    }

    /// Abort the named call. Aborting drops the outstanding HTTP request, which closes the
    /// connection, which is what tells opman to take the card down — a cancelled turn must
    /// not leave a question on screen with nothing left to answer.
    fn cancel(&mut self, id: &Value) {
        if let Some(handle) = self.0.remove(&key(id)) {
            handle.abort();
        }
    }

    /// Drop handles for calls that have already answered, so a long session's map stays
    /// the size of what is actually outstanding.
    fn prune(&mut self) {
        self.0.retain(|_, handle| !handle.is_finished());
    }
}

/// JSON-RPC ids may be numbers or strings; compare them as written.
fn key(id: &Value) -> String {
    id.to_string()
}

pub async fn run_mcp_ask_bridge(project_path: PathBuf) -> anyhow::Result<()> {
    let context = Context {
        loopback: Loopback::load(),
        session: std::env::var(SESSION_ENV).ok().filter(|s| !s.is_empty()),
        directory: std::fs::canonicalize(&project_path)
            .unwrap_or(project_path)
            .to_string_lossy()
            .into_owned(),
    };
    run_ask_bridge(
        context,
        tokio::io::stdin(),
        Arc::new(Mutex::new(tokio::io::stdout())),
    )
    .await
}

async fn run_ask_bridge<R, W>(
    context: Context,
    reader: R,
    stdout: Arc<Mutex<W>>,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let context = Arc::new(context);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut calls = JoinSet::new();
    let mut in_flight = InFlight::default();

    loop {
        while calls.try_join_next().is_some() {}
        in_flight.prune();
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let request: RpcRequest = match serde_json::from_str(trimmed) {
            Ok(request) => request,
            Err(error) => {
                let message = format!("Parse error: {error}");
                write_rpc(&stdout, &rpc_error(Value::Null, -32700, &message)).await;
                continue;
            }
        };
        dispatch(request, &context, &stdout, &mut calls, &mut in_flight).await;
    }

    // Two different things can be in flight when the pipe closes, and they want opposite
    // treatment: a call that has already been answered still owes its result, while a
    // question sitting on screen can never be answered now and must let go of its HTTP
    // request so opman takes the card down. So: a short grace period to drain the first,
    // then abort whatever is left.
    let drain = async { while calls.join_next().await.is_some() {} };
    if tokio::time::timeout(SHUTDOWN_GRACE, drain).await.is_err() {
        calls.abort_all();
        while calls.join_next().await.is_some() {}
    }
    Ok(())
}

async fn dispatch<W>(
    request: RpcRequest,
    context: &Arc<Context>,
    stdout: &Arc<Mutex<W>>,
    calls: &mut JoinSet<()>,
    in_flight: &mut InFlight,
) where
    W: AsyncWrite + Unpin + Send + 'static,
{
    match request.method.as_str() {
        "initialize" => {
            let result = json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "opman-ask", "version": env!("CARGO_PKG_VERSION") },
            });
            write_rpc(stdout, &rpc_result(request.id, result)).await;
        }
        "notifications/initialized" => {}
        "notifications/cancelled" => {
            if let Some(id) = request.params.as_ref().and_then(|p| p.get("requestId")) {
                in_flight.cancel(id);
            }
        }
        "tools/list" => {
            let result = json!({ "tools": tools::definitions() });
            write_rpc(stdout, &rpc_result(request.id, result)).await;
        }
        "tools/call" => spawn_call(request, context, stdout, calls, in_flight),
        other => {
            let message = format!("Method not found: {other}");
            write_rpc(stdout, &rpc_error(request.id, -32601, &message)).await;
        }
    }
}

/// Spawned, never awaited here: the wait is a human's, and holding the read loop would
/// stall the very `notifications/cancelled` that is meant to end it.
fn spawn_call<W>(
    request: RpcRequest,
    context: &Arc<Context>,
    stdout: &Arc<Mutex<W>>,
    calls: &mut JoinSet<()>,
    in_flight: &mut InFlight,
) where
    W: AsyncWrite + Unpin + Send + 'static,
{
    let named = request
        .params
        .as_ref()
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let token = request
        .params
        .as_ref()
        .and_then(|p| p.pointer("/_meta/progressToken"))
        .cloned();
    let context = context.clone();
    let stdout = stdout.clone();
    let id = request.id.clone();
    let handle = calls.spawn(async move {
        let text = if named == tools::TOOL_NAME {
            run_tool(&context, &stdout, request.params.as_ref(), token).await
        } else {
            format!("Unknown tool: {named}")
        };
        let result = json!({ "content": [{ "type": "text", "text": text }] });
        write_rpc(&stdout, &rpc_result(id, result)).await;
    });
    in_flight.track(&request.id, handle);
}

async fn run_tool<W>(
    context: &Context,
    stdout: &Arc<Mutex<W>>,
    params: Option<&Value>,
    token: Option<Value>,
) -> String
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    let questions = match tools::questions(params) {
        Ok(questions) => questions,
        Err(complaint) => return complaint,
    };
    let ask = tools::ask(
        context.loopback.as_ref(),
        context.session.as_deref(),
        &context.directory,
        questions,
    );
    tokio::select! {
        answer = ask => answer,
        // Diverges: the ticker exists only to be dropped when the answer lands.
        never = progress::tick_until_dropped(stdout, token) => match never {},
    }
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// One line per message, flushed: the client is reading a pipe, not a stream it can poll.
async fn write_rpc<W>(stdout: &Arc<Mutex<W>>, message: &Value)
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    let Ok(mut encoded) = serde_json::to_vec(message) else {
        return;
    };
    encoded.push(b'\n');
    let mut stdout = stdout.lock().await;
    let _ = stdout.write_all(&encoded).await;
    let _ = stdout.flush().await;
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
