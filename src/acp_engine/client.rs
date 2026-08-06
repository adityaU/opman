//! opman as an ACP *client*: the agent→opman half of the protocol.
//!
//! Two things arrive here. Notifications (`session/update`) are rendering, handed to
//! [`super::render`]. Requests are the agent asking opman to do something and waiting on
//! the answer — above all `session/request_permission`, which replaces the `PreToolUse`
//! hook the `claude -p` engine needed: permission is now a first-class protocol round-trip
//! rather than a subprocess calling back into opman over HTTP.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use futures::future::BoxFuture;
use serde_json::{json, Value};

use super::jsonrpc::Handler;
use super::AcpEngine;
use crate::claude_engine::PendingReply;

/// How long a permission request waits for a human before it is treated as unanswered.
const PERMISSION_TIMEOUT: Duration = Duration::from_secs(3600);

/// Routes inbound ACP traffic for one agent connection.
pub struct Client {
    engine: Arc<AcpEngine>,
}

impl Client {
    pub fn new(engine: Arc<AcpEngine>) -> Arc<Self> {
        Arc::new(Self { engine })
    }
}

impl Handler for Client {
    fn request(
        self: Arc<Self>,
        method: String,
        params: Value,
    ) -> BoxFuture<'static, Result<Value>> {
        Box::pin(async move {
            match method.as_str() {
                "session/request_permission" => request_permission(&self.engine, &params).await,
                "fs/read_text_file" => read_text_file(&self.engine, &params),
                "fs/write_text_file" => write_text_file(&self.engine, &params),
                // Anything else is a capability opman never advertised. Say so plainly:
                // a clear error lets the agent fall back to its own tools, where silence
                // would hang the turn.
                other => bail!("opman does not implement `{other}`"),
            }
        })
    }

    fn notify(self: Arc<Self>, method: String, params: Value) {
        if method != "session/update" {
            return;
        }
        let Some(session_id) = self.engine.opman_session(&params) else {
            return;
        };
        if let Some(update) = params.get("update") {
            super::render::apply(&self.engine, &session_id, update);
        }
    }
}

/// Surface a permission request in opman and block the agent on the user's answer.
async fn request_permission(engine: &Arc<AcpEngine>, params: &Value) -> Result<Value> {
    let Some(session_id) = engine.opman_session(params) else {
        bail!("permission request for an unknown session");
    };
    let tool_call = params.get("toolCall").cloned().unwrap_or(json!({}));
    let options = params
        .get("options")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let tool = tool_label(&tool_call);

    // A tool the user already blessed for this session never asks twice.
    if engine.is_always_allowed(&session_id, &tool) {
        if let Some(id) = option_for(&options, "allow") {
            return Ok(selected(&id));
        }
    }

    let request_id = super::rand_id("perm");
    let dir = engine
        .get_session(&session_id)
        .map(|s| s.directory)
        .unwrap_or_default();
    engine.emit(
        &dir,
        "permission.asked",
        json!({
            "id": request_id,
            "sessionID": session_id,
            "permission": tool,
            "patterns": patterns(&tool_call),
            "metadata": tool_call.get("rawInput").cloned().unwrap_or(json!({})),
        }),
    );

    let rx = engine.register_pending(&request_id);
    let reply = match tokio::time::timeout(PERMISSION_TIMEOUT, rx).await {
        Ok(Ok(PendingReply::Permission(reply))) => reply,
        // Timed out or dismissed. Clear the waiter so a late answer cannot resolve a
        // request the agent has already been told about.
        _ => {
            engine.resolve_pending(&request_id, PendingReply::Reject);
            "reject".to_string()
        }
    };
    if reply == "always" {
        engine.add_allowed_tool(&session_id, &tool);
    }
    let intent = match reply.as_str() {
        "always" => "allow_always",
        "reject" => "reject",
        _ => "allow",
    };
    match option_for(&options, intent) {
        Some(id) => Ok(selected(&id)),
        // The agent offered no option matching the user's choice. Cancelling is the only
        // truthful outcome — never silently substitute an allow for a reject.
        None => Ok(json!({ "outcome": { "outcome": "cancelled" } })),
    }
}

fn selected(option_id: &str) -> Value {
    json!({ "outcome": { "outcome": "selected", "optionId": option_id } })
}

/// Pick the option id matching an intent, preferring an exact `kind` and falling back to
/// the other option of the same polarity (an agent may offer only "always", or only "once").
fn option_for(options: &[Value], intent: &str) -> Option<String> {
    let wanted: &[&str] = match intent {
        "allow_always" => &["allow_always", "allow_once"],
        "reject" => &["reject_once", "reject_always"],
        _ => &["allow_once", "allow_always"],
    };
    for kind in wanted {
        let found = options
            .iter()
            .find(|o| o.get("kind").and_then(Value::as_str) == Some(*kind))
            .and_then(|o| o.get("optionId"))
            .and_then(Value::as_str);
        if let Some(id) = found {
            return Some(id.to_string());
        }
    }
    None
}

/// What the user is being asked to approve: the agent's tool name when it names one,
/// else the human-readable title.
fn tool_label(tool_call: &Value) -> String {
    let named = tool_call
        .get("_meta")
        .and_then(|m| m.get("claudeCode"))
        .and_then(|c| c.get("toolName"))
        .and_then(Value::as_str);
    named
        .or_else(|| tool_call.get("title").and_then(Value::as_str))
        .unwrap_or("tool")
        .to_string()
}

/// The concrete targets of the call (paths, command line), shown under the prompt so the
/// user approves a specific action rather than a tool name.
fn patterns(tool_call: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(locations) = tool_call.get("locations").and_then(Value::as_array) {
        out.extend(
            locations
                .iter()
                .filter_map(|l| l.get("path").and_then(Value::as_str))
                .map(str::to_string),
        );
    }
    let Some(input) = tool_call.get("rawInput") else {
        return out;
    };
    for key in ["file_path", "path", "notebook_path", "command"] {
        if let Some(value) = input.get(key).and_then(Value::as_str) {
            if !out.iter().any(|p| p == value) {
                out.push(value.to_string());
            }
        }
    }
    out
}

/// `fs/read_text_file`. Only reachable when the agent's config advertises the capability.
fn read_text_file(engine: &Arc<AcpEngine>, params: &Value) -> Result<Value> {
    if !engine.agent.client_caps.read_text_file {
        bail!("opman does not implement `fs/read_text_file`");
    }
    let path = require_path(params)?;
    let content = std::fs::read_to_string(&path)?;
    let line = params.get("line").and_then(Value::as_u64);
    let limit = params.get("limit").and_then(Value::as_u64);
    Ok(json!({ "content": slice_lines(&content, line, limit) }))
}

/// `fs/write_text_file`. Creates parent directories, since an agent asking opman to write
/// a new file should not have to ask it to make the directory first.
fn write_text_file(engine: &Arc<AcpEngine>, params: &Value) -> Result<Value> {
    if !engine.agent.client_caps.write_text_file {
        bail!("opman does not implement `fs/write_text_file`");
    }
    let path = require_path(params)?;
    let content = params
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    Ok(json!({}))
}

/// ACP requires absolute paths; enforce it rather than resolving against an arbitrary cwd.
fn require_path(params: &Value) -> Result<std::path::PathBuf> {
    let raw = params
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let path = std::path::PathBuf::from(raw);
    if raw.is_empty() || !path.is_absolute() {
        bail!("`path` must be an absolute path");
    }
    Ok(path)
}

/// Apply ACP's optional 1-based `line` offset and `limit` window.
fn slice_lines(content: &str, line: Option<u64>, limit: Option<u64>) -> String {
    if line.is_none() && limit.is_none() {
        return content.to_string();
    }
    let skip = line.unwrap_or(1).saturating_sub(1) as usize;
    let take = limit.map(|l| l as usize).unwrap_or(usize::MAX);
    content
        .lines()
        .skip(skip)
        .take(take)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod client_tests;
