//! PreToolUse hook callback for the `claude -p` engine: surfaces permission prompts and
//! AskUserQuestion to opman and blocks until the user replies (same contract as the
//! background engine). The hook is a thin `opman claude-hook` relay configured via
//! `--settings`.

use std::time::Duration;

use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use super::routes::Engine;
use crate::claude_engine::PendingReply;

const GATED_TOOLS: &[&str] = &["Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"];
const EDIT_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "NotebookEdit"];

fn hook_allow() -> Value {
    json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "allow" } })
}
fn hook_deny(reason: &str) -> Value {
    json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": reason } })
}
fn rand_request_id() -> String {
    let n: u128 = rand::random();
    format!("perm_{n:032x}")
}

fn permission_patterns(tool_input: &Value) -> Vec<String> {
    let mut out = vec![];
    for key in ["file_path", "path", "notebook_path"] {
        if let Some(p) = tool_input.get(key).and_then(|v| v.as_str()) {
            out.push(p.to_string());
        }
    }
    if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
        out.push(cmd.to_string());
    }
    out
}

fn build_questions(tool_input: &Value) -> Value {
    let Some(qs) = tool_input.get("questions").and_then(|q| q.as_array()) else { return json!([]) };
    let mapped: Vec<Value> = qs
        .iter()
        .map(|q| {
            let options: Vec<Value> = q
                .get("options")
                .and_then(|o| o.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|o| {
                            json!({
                                "label": o.get("label").and_then(|s| s.as_str()).unwrap_or(""),
                                "description": o.get("description").and_then(|s| s.as_str()).unwrap_or(""),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "question": q.get("question").and_then(|s| s.as_str()).unwrap_or(""),
                "header": q.get("header").and_then(|s| s.as_str()).unwrap_or(""),
                "options": options,
                "multiple": q.get("multiSelect").and_then(|b| b.as_bool()).unwrap_or(false),
                "custom": true,
            })
        })
        .collect();
    json!(mapped)
}

fn format_answers(tool_input: &Value, answers: &[Vec<String>]) -> String {
    let qs = tool_input.get("questions").and_then(|q| q.as_array()).cloned().unwrap_or_default();
    let mut lines = vec!["[USER ANSWER] The user answered your question(s):".to_string()];
    for (i, ans) in answers.iter().enumerate() {
        let q = qs.get(i).and_then(|q| q.get("question")).and_then(|s| s.as_str()).unwrap_or("(question)");
        lines.push(format!("  • {q} → {}", ans.join(", ")));
    }
    lines.push("Treat these as the answers and continue; do NOT ask again.".to_string());
    lines.join("\n")
}

pub(super) async fn internal_ask(State(engine): State<Engine>, body: Json<Value>) -> Json<Value> {
    let input = body.0;
    let claude_uuid = input.get("session_id").and_then(|s| s.as_str()).unwrap_or("");
    let cwd = input.get("cwd").and_then(|s| s.as_str()).unwrap_or("");
    let tool = input.get("tool_name").and_then(|s| s.as_str()).unwrap_or("");
    let tool_input = input.get("tool_input").cloned().unwrap_or(json!({}));

    let session_id = engine
        .session_id_for_claude_uuid(claude_uuid)
        .or_else(|| engine.list_for_dir(cwd).into_iter().next().map(|s| s.id));
    let Some(session_id) = session_id else { return Json(hook_allow()) };
    let dir = engine.get_session(&session_id).map(|s| s.directory).unwrap_or_else(|| cwd.to_string());
    let mode = engine.effective_mode(&session_id);

    if tool == "AskUserQuestion" {
        let id = rand_request_id();
        engine.emit(
            &dir,
            "question.asked",
            json!({ "id": id, "sessionID": session_id, "questions": build_questions(&tool_input) }),
        );
        let rx = engine.register_pending(&id);
        return match tokio::time::timeout(Duration::from_secs(3600), rx).await {
            Ok(Ok(PendingReply::Question(answers))) => Json(hook_deny(&format_answers(&tool_input, &answers))),
            _ => {
                engine.resolve_pending(&id, PendingReply::Reject);
                Json(hook_deny("[USER] The question was dismissed; pick a reasonable default and continue."))
            }
        };
    }

    let gated = GATED_TOOLS.contains(&tool);
    if mode == "bypassPermissions" || !gated {
        return Json(hook_allow());
    }
    if mode == "acceptEdits" && EDIT_TOOLS.contains(&tool) {
        return Json(hook_allow());
    }
    if mode == "plan" && EDIT_TOOLS.contains(&tool) {
        return Json(hook_deny("[USER] Plan mode is active — do not modify files."));
    }
    if engine.is_always_allowed(&session_id, tool) {
        return Json(hook_allow());
    }

    let id = rand_request_id();
    engine.emit(
        &dir,
        "permission.asked",
        json!({ "id": id, "sessionID": session_id, "permission": tool, "patterns": permission_patterns(&tool_input), "metadata": tool_input }),
    );
    let rx = engine.register_pending(&id);
    match tokio::time::timeout(Duration::from_secs(3600), rx).await {
        Ok(Ok(PendingReply::Permission(reply))) => match reply.as_str() {
            "always" => {
                engine.add_allowed_tool(&session_id, tool);
                Json(hook_allow())
            }
            "reject" => Json(hook_deny("[USER] Permission denied by the user.")),
            _ => Json(hook_allow()),
        },
        _ => {
            engine.resolve_pending(&id, PendingReply::Reject);
            Json(hook_deny("[USER] Permission request was not answered."))
        }
    }
}

pub(super) async fn permission_reply(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let reply = body.get("reply").and_then(|r| r.as_str()).unwrap_or("once").to_string();
    engine.resolve_pending(&id, PendingReply::Permission(reply));
    Json(json!({ "ok": true }))
}

pub(super) async fn question_reply(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let answers: Vec<Vec<String>> =
        body.get("answers").and_then(|a| serde_json::from_value(a.clone()).ok()).unwrap_or_default();
    engine.resolve_pending(&id, PendingReply::Question(answers));
    Json(json!({ "ok": true }))
}

pub(super) async fn question_reject(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    engine.resolve_pending(&id, PendingReply::Reject);
    Json(json!({ "ok": true }))
}

#[cfg(test)]
#[path = "routes_hook_tests.rs"]
mod routes_hook_tests;

#[cfg(test)]
#[path = "routes_hook_blocking_tests.rs"]
mod routes_hook_blocking_tests;
