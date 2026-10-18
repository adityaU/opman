//! Run one slash command on an ACP agent and read its answer, then stop the agent.
//!
//! This is for reading something the agent only offers as a command, such as Antigravity's
//! `/usage`. It starts its own short-lived process, so it never touches a running session.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout};

use super::config::AgentConfig;
use super::failure::StderrTail;
use super::jsonrpc::AUTH_REQUIRED;
use super::launch;

/// Numbers each call's folder, so one call cleaning up cannot remove another's.
static NEXT_DIR: AtomicU64 = AtomicU64::new(1);

/// What the agent said, and what it told us about itself when the session opened.
#[derive(Debug, PartialEq)]
pub struct Answer {
    /// The `session/new` result, which can carry agent-specific facts such as the plan.
    pub session: Value,
    /// The agent's message text, in order.
    pub text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    /// The agent wants a login first.
    LoginRequired,
    Other(String),
}

impl From<String> for Failure {
    fn from(reason: String) -> Self {
        Self::Other(reason)
    }
}

/// Start `agent`, send it `command` as a prompt, and collect its reply. The whole exchange
/// has `limit` to finish.
pub async fn ask(agent: &AgentConfig, command: &str, limit: Duration) -> Result<Answer, Failure> {
    let number = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("opman-oneshot-{}-{number}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let outcome = start(agent, command, &dir.to_string_lossy(), limit).await;
    let _ = std::fs::remove_dir(&dir);
    outcome
}

async fn start(
    agent: &AgentConfig,
    command: &str,
    dir: &str,
    limit: Duration,
) -> Result<Answer, Failure> {
    let mut child = launch::spawn(agent, dir).map_err(|e| format!("{e:#}"))?;
    let tail = StderrTail::capture(&mut child);
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Err("the agent process has no stdin/stdout pipes".to_string().into());
    };
    let exchange = exchange(stdin, stdout, command, dir);
    let outcome = match tokio::time::timeout(limit, exchange).await {
        Ok(outcome) => outcome,
        Err(_) => Err(format!("the agent did not answer within {} seconds", limit.as_secs()).into()),
    };
    let _ = child.kill().await;
    match outcome {
        Err(Failure::Other(reason)) => {
            let stderr = tail.text().await;
            let more = if stderr.is_empty() {
                String::new()
            } else {
                format!("\nAgent stderr (last part):\n{stderr}")
            };
            Err(Failure::Other(format!("{reason}{more}")))
        }
        other => other,
    }
}

async fn exchange(
    mut stdin: ChildStdin,
    stdout: ChildStdout,
    command: &str,
    dir: &str,
) -> Result<Answer, Failure> {
    let mut lines = BufReader::new(stdout).lines();
    let init = json!({ "protocolVersion": 1, "clientCapabilities": {} });
    call(&mut stdin, &mut lines, 1, "initialize", init, &mut String::new()).await?;
    let new = json!({ "cwd": dir, "mcpServers": [] });
    let session = call(&mut stdin, &mut lines, 2, "session/new", new, &mut String::new()).await?;
    let Some(id) = session.get("sessionId").and_then(Value::as_str) else {
        return Err("`session/new` returned no sessionId".to_string().into());
    };
    let prompt = json!({ "sessionId": id, "prompt": [{ "type": "text", "text": command }] });
    let mut text = String::new();
    call(&mut stdin, &mut lines, 3, "session/prompt", prompt, &mut text).await?;
    Ok(Answer { session, text })
}

/// Send one request and read frames until its reply, adding message text to `text`.
async fn call(
    stdin: &mut ChildStdin,
    lines: &mut Lines<BufReader<ChildStdout>>,
    id: u64,
    method: &str,
    params: Value,
    text: &mut String,
) -> Result<Value, Failure> {
    let mut frame = serde_json::to_string(&json!({
        "jsonrpc": "2.0", "id": id, "method": method, "params": params
    }))
    .map_err(|e| e.to_string())?;
    frame.push('\n');
    stdin.write_all(frame.as_bytes()).await.map_err(|e| format!("cannot write to the agent: {e}"))?;
    stdin.flush().await.map_err(|e| format!("cannot write to the agent: {e}"))?;
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => return Err(format!("the agent closed its output during `{method}`").into()),
            Err(e) => return Err(format!("cannot read from the agent: {e}").into()),
        };
        let Ok(reply) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if reply.get("method").and_then(Value::as_str) == Some("session/update") {
            add_message_text(&reply, text);
            continue;
        }
        if reply.get("id").and_then(Value::as_u64) != Some(id) || reply.get("method").is_some() {
            continue;
        }
        return outcome(method, &reply);
    }
}

fn add_message_text(update: &Value, text: &mut String) {
    let Some(body) = update.pointer("/params/update") else {
        return;
    };
    if body.get("sessionUpdate").and_then(Value::as_str) != Some("agent_message_chunk") {
        return;
    }
    if let Some(chunk) = body.pointer("/content/text").and_then(Value::as_str) {
        text.push_str(chunk);
    }
}

fn outcome(method: &str, reply: &Value) -> Result<Value, Failure> {
    let Some(error) = reply.get("error") else {
        return Ok(reply.get("result").cloned().unwrap_or(Value::Null));
    };
    if error.get("code").and_then(Value::as_i64) == Some(AUTH_REQUIRED) {
        return Err(Failure::LoginRequired);
    }
    let message = error.get("message").and_then(Value::as_str).unwrap_or("unknown error");
    Err(format!("`{method}` failed: {message}").into())
}

#[cfg(test)]
#[path = "oneshot_tests.rs"]
mod oneshot_tests;
