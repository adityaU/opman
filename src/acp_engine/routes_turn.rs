//! Turn, transcript and permission-reply handlers.
//!
//! Split from [`super::routes`], which owns the router and session CRUD. Notably absent is an
//! `/internal/ask` hook endpoint: permission is an ACP round-trip now, not an HTTP callback
//! from a spawned CLI.

use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use super::attach::Prompt;
use super::routes::Engine;
use super::turn;
use crate::claude_engine::{claude_cli, jsonl, PendingReply};

// ── turns ───────────────────────────────────────────────────────────

pub(super) async fn send_message(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    if let Some(model) = body
        .get("model")
        .and_then(|m| m.get("modelID"))
        .and_then(Value::as_str)
    {
        engine.set_model(&id, model);
    }
    if let Some(agent) = body.get("agent").and_then(Value::as_str) {
        engine.set_agent(&id, agent);
    }
    if let Some(effort) = body.get("effort").and_then(Value::as_str) {
        engine.set_effort(&id, effort);
    }
    if let Some(permission) = body.get("permission").and_then(Value::as_str) {
        engine.set_permission_mode(&id, permission);
    }
    // An attachment with no words is still a turn, so emptiness is judged on the whole
    // prompt rather than on its text.
    let prompt = Prompt::from_body(&body.0);
    if !prompt.is_empty() {
        tokio::spawn(turn::prompt(engine, id, prompt));
    }
    Json(json!({ "ok": true }))
}

/// A slash command is just a prompt: ACP agents parse their own commands, and
/// `available_commands_update` is how they say which ones exist.
pub(super) async fn session_command(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let command = body.get("command").and_then(Value::as_str).unwrap_or("");
    let args = body.get("arguments").and_then(Value::as_str).unwrap_or("");
    let text = if args.is_empty() {
        format!("/{command}")
    } else {
        format!("/{command} {args}")
    };
    tokio::spawn(turn::prompt(engine, id, Prompt::text(text)));
    Json(json!({ "ok": true }))
}

pub(super) async fn abort(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    turn::abort(engine, &id).await;
    Json(json!({ "ok": true }))
}

// ── messages / todos ────────────────────────────────────────────────

/// Rendered messages. Live sessions are served from the folded transcript — replaying the
/// agent's history first if this is the first read since a restart, which is the only way
/// an old conversation comes back. Subagent rows have no ACP session of their own, so they
/// come from the agent's on-disk transcript when the config says opman can read it.
pub(super) async fn get_messages(
    State(engine): State<Engine>,
    Path(id): Path<String>,
) -> Json<Value> {
    let entry = engine.get_session(&id);
    let is_subagent = entry.as_ref().map(|e| e.is_subagent).unwrap_or(true);
    if is_subagent && engine.agent.subagent_transcripts {
        return Json(Value::Array(subagent_messages(&id)));
    }
    Json(Value::Array(super::history::messages(&engine, &id).await))
}

fn subagent_messages(id: &str) -> Vec<Value> {
    let Some(path) = claude_cli::locate_subagent_jsonl(id) else {
        return Vec::new();
    };
    jsonl::parse_file(&path, id)
        .messages
        .iter()
        .map(|m| m.to_value())
        .collect()
}

pub(super) async fn get_todos(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    Json(Value::Array(engine.todos(&id)))
}

// ── permission replies ──────────────────────────────────────────────

/// Broadcast that a request was resolved so every mirror drops its pending card. The reply
/// endpoints report whether *this* engine owned the request: the runner registry fans a
/// reply out across all engines, so an unconditional `ok` would stop the fan-out at the
/// first engine asked rather than the one that asked the question.
fn emit_resolved(engine: &Engine, id: &str, event: &str) {
    engine.emit(
        "",
        event,
        json!({ "id": id, "requestID": id, "sessionID": "" }),
    );
}

pub(super) async fn permission_reply(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let reply = body
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or("once")
        .to_string();
    let owned = engine.resolve_pending(&id, PendingReply::Permission(reply));
    if owned {
        emit_resolved(&engine, &id, "permission.replied");
    }
    Json(json!({ "ok": owned }))
}

pub(super) async fn question_reply(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let answers: Vec<Vec<String>> = body
        .get("answers")
        .and_then(|a| serde_json::from_value(a.clone()).ok())
        .unwrap_or_default();
    let owned = engine.resolve_pending(&id, PendingReply::Question(answers));
    if owned {
        emit_resolved(&engine, &id, "question.replied");
    }
    Json(json!({ "ok": owned }))
}

pub(super) async fn question_reject(
    State(engine): State<Engine>,
    Path(id): Path<String>,
) -> Json<Value> {
    let owned = engine.resolve_pending(&id, PendingReply::Reject);
    if owned {
        emit_resolved(&engine, &id, "question.rejected");
    }
    Json(json!({ "ok": owned }))
}
