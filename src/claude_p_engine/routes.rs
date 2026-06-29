//! opencode-compatible REST + SSE routes for the `claude -p` engine. Mirrors the
//! background engine's contract so the opman web layer proxies to it unchanged.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::{process, session_info, ClaudePEngine};
use crate::claude_engine::{claude_cli, jsonl, PendingReply};

type Engine = Arc<ClaudePEngine>;

pub fn router(engine: Engine) -> Router {
    Router::new()
        .route("/info", get(info))
        .route("/health", get(health))
        .route("/event", get(event_stream))
        .route("/session", get(list_sessions).post(create_session))
        .route("/session/status", get(session_status))
        .route("/provider", get(provider))
        .route("/command", get(command_list))
        .route("/agent", get(agent_list))
        .route(
            "/session/{id}",
            get(get_session).patch(rename_session).delete(delete_session),
        )
        .route("/session/{id}/message", get(get_messages).post(send_message))
        .route("/session/{id}/prompt_async", post(send_message))
        .route("/session/{id}/abort", post(abort))
        .route("/session/{id}/todo", get(get_todos))
        .route("/session/{id}/command", post(session_command))
        .route("/session/{id}/revert", post(noop_ok))
        .route("/session/{id}/unrevert", post(noop_ok))
        .route("/session/{id}/share", post(noop_obj))
        .route("/tui/select-session", post(noop_ok))
        .route("/permission/{id}/reply", post(permission_reply))
        .route("/question/{id}/reply", post(question_reply))
        .route("/question/{id}/reject", post(question_reject))
        .route("/internal/ask", post(internal_ask))
        .with_state(engine)
}

// ── helpers ─────────────────────────────────────────────────────────

fn dir_header(headers: &HeaderMap) -> String {
    headers
        .get("x-opencode-directory")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn extract_text(body: &Value) -> String {
    if let Some(parts) = body.get("parts").and_then(|p| p.as_array()) {
        let joined = parts
            .iter()
            .filter_map(|p| {
                let t = p.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                (t == "text").then(|| p.get("text").and_then(|t| t.as_str())).flatten()
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !joined.is_empty() {
            return joined;
        }
    }
    body.get("text")
        .and_then(|t| t.as_str())
        .or_else(|| body.get("prompt").and_then(|t| t.as_str()))
        .unwrap_or("")
        .to_string()
}

const PERMISSION_MODES: &[&str] =
    &["default", "acceptEdits", "auto", "bypassPermissions", "dontAsk", "plan"];
const GATED_TOOLS: &[&str] = &["Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"];
const EDIT_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "NotebookEdit"];

/// Apply a runtime control command (`/agent`, `/permission-mode`); true = consumed.
fn handle_control_command(engine: &Engine, session_id: &str, text: &str) -> bool {
    let t = text.trim();
    if t == "/agent" || t == "/agents" {
        return true;
    }
    if let Some(name) = t.strip_prefix("/agent ") {
        let name = name.trim();
        if !name.is_empty() {
            engine.set_agent(session_id, name);
            if let Some(s) = engine.get_session(session_id) {
                engine.emit(
                    &s.directory,
                    "tui.toast.show",
                    json!({ "message": format!("Claude agent: {name}"), "variant": "info" }),
                );
            }
        }
        return true;
    }
    let rest = t
        .strip_prefix("/permission-mode")
        .or_else(|| t.strip_prefix("/perm-mode"))
        .or_else(|| t.strip_prefix("/perm"));
    if let Some(rest) = rest {
        let mode = rest.trim();
        match PERMISSION_MODES.iter().find(|m| m.eq_ignore_ascii_case(mode)).copied() {
            Some(m) => engine.set_permission_mode(session_id, m),
            None => {
                if let Some(s) = engine.get_session(session_id) {
                    engine.emit(
                        &s.directory,
                        "tui.toast.show",
                        json!({ "message": format!("Unknown permission mode '{mode}'"), "variant": "error" }),
                    );
                }
            }
        }
        return true;
    }
    false
}

/// Dispatch a user turn: control command, or push to the running `claude -p` process.
fn dispatch_turn(engine: Engine, session_id: String, text: String) {
    if text.trim().is_empty() {
        return;
    }
    if handle_control_command(&engine, &session_id, &text) {
        return;
    }
    tokio::spawn(process::send(engine, session_id, text));
}

// ── basic handlers ──────────────────────────────────────────────────

async fn info(headers: HeaderMap) -> Json<Value> {
    Json(json!({ "directory": dir_header(&headers), "version": claude_cli::version() }))
}

async fn health() -> &'static str {
    "ok"
}

async fn noop_ok() -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn noop_obj() -> Json<Value> {
    Json(json!({}))
}

async fn list_sessions(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    let dir = dir_header(&headers);
    let arr: Vec<Value> = engine.list_for_dir(&dir).iter().map(session_info).collect();
    Json(Value::Array(arr))
}

async fn create_session(
    State(engine): State<Engine>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Json<Value> {
    let dir = dir_header(&headers);
    let body = body.map(|b| b.0).unwrap_or(Value::Null);
    let parent = body.get("parentID").and_then(|p| p.as_str()).unwrap_or("").to_string();
    let title = body.get("title").and_then(|t| t.as_str()).unwrap_or("New session").to_string();
    Json(session_info(&engine.create_session(&dir, &parent, &title)))
}

async fn get_session(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    match engine.get_session(&id) {
        Some(e) => Json(session_info(&e)),
        None => Json(json!({ "id": id })),
    }
}

async fn rename_session(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    if let Some(title) = body.get("title").and_then(|t| t.as_str()) {
        engine.rename_session(&id, title);
    }
    match engine.get_session(&id) {
        Some(e) => Json(session_info(&e)),
        None => Json(json!({ "id": id })),
    }
}

async fn delete_session(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    engine.delete_session(&id).await;
    Json(json!({ "ok": true }))
}

async fn session_status(State(engine): State<Engine>) -> Json<Value> {
    let mut map = serde_json::Map::new();
    for (id, busy) in engine.busy_map() {
        if busy {
            map.insert(id, json!({ "type": "busy" }));
        }
    }
    Json(Value::Object(map))
}

async fn send_message(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    if let Some(model_id) = body.get("model").and_then(|m| m.get("modelID")).and_then(|s| s.as_str()) {
        engine.set_model(&id, model_id);
    }
    if let Some(agent) = body.get("agent").and_then(|a| a.as_str()) {
        engine.set_agent(&id, agent);
    }
    dispatch_turn(engine, id, extract_text(&body.0));
    Json(json!({ "ok": true }))
}

async fn session_command(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let command = body.get("command").and_then(|c| c.as_str()).unwrap_or("");
    let args = body.get("arguments").and_then(|a| a.as_str()).unwrap_or("");
    let text = if args.is_empty() { format!("/{command}") } else { format!("/{command} {args}") };
    dispatch_turn(engine, id, text);
    Json(json!({ "ok": true }))
}

async fn abort(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    process::abort(engine, &id).await;
    Json(json!({ "ok": true }))
}

// ── messages / todos ────────────────────────────────────────────────

fn render_transcript(uuid: &str, session_id: &str) -> Vec<Value> {
    let Some(path) = claude_cli::locate_jsonl(uuid) else { return vec![] };
    let mut parsed = jsonl::parse_file(&path, session_id);
    jsonl::enrich_subagents(&mut parsed);
    jsonl::enrich_background_tasks(&mut parsed);
    parsed.messages.iter().map(|m| m.to_value()).collect()
}

async fn get_messages(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    let Some(entry) = engine.get_session(&id) else {
        // A subagent child id the web UI backfills on reload.
        if let Some(path) = claude_cli::locate_subagent_jsonl(&id) {
            let parsed = jsonl::parse_file(&path, &id);
            return Json(Value::Array(parsed.messages.iter().map(|m| m.to_value()).collect()));
        }
        return Json(Value::Array(vec![]));
    };
    let Some(uuid) = entry.claude_uuid else { return Json(Value::Array(vec![])) };
    Json(Value::Array(render_transcript(&uuid, &id)))
}

async fn get_todos(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    let mut todos: Vec<Value> = vec![];
    if let Some(uuid) = engine.get_session(&id).and_then(|s| s.claude_uuid) {
        if let Some(path) = claude_cli::locate_jsonl(&uuid) {
            let parsed = jsonl::parse_file(&path, &id);
            for msg in &parsed.messages {
                for part in &msg.parts {
                    if part.get("tool").and_then(|t| t.as_str()) == Some("TodoWrite") {
                        if let Some(items) =
                            part.get("state").and_then(|s| s.get("input")).and_then(|i| i.get("todos")).and_then(|t| t.as_array())
                        {
                            todos = items.clone();
                        }
                    }
                }
            }
        }
    }
    Json(Value::Array(todos))
}

// ── provider / commands / agents ────────────────────────────────────

async fn provider() -> Json<Value> {
    let model = |id: &str, name: &str| {
        json!({ "id": id, "providerID": "anthropic", "name": name, "limit": { "context": 200_000, "output": 64_000 } })
    };
    Json(json!({
        "all": [{
            "id": "anthropic", "name": "Anthropic",
            "models": {
                "claude-opus-4-8": model("claude-opus-4-8", "Claude Opus 4.8"),
                "claude-sonnet-4-6": model("claude-sonnet-4-6", "Claude Sonnet 4.6"),
                "claude-haiku-4-5-20251001": model("claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
            }
        }],
        "connected": ["anthropic"],
        "default": { "anthropic": "claude-sonnet-4-6" },
    }))
}

async fn init_for_dir(engine: &Engine, dir: &str) -> claude_cli::InitInfo {
    if let Some(info) = engine.cached_init(dir) {
        return info;
    }
    let d = dir.to_string();
    let info = tokio::task::spawn_blocking(move || claude_cli::introspect(&d)).await.unwrap_or_default();
    engine.set_cached_init(dir, info.clone());
    info
}

async fn command_list(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    let dir = dir_header(&headers);
    if dir.is_empty() {
        return Json(Value::Array(vec![]));
    }
    let arr: Vec<Value> =
        init_for_dir(&engine, &dir).await.commands.iter().map(|name| json!({ "name": name })).collect();
    Json(Value::Array(arr))
}

async fn agent_list(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    let dir = dir_header(&headers);
    if dir.is_empty() {
        return Json(Value::Array(vec![]));
    }
    let arr: Vec<Value> = init_for_dir(&engine, &dir)
        .await
        .agents
        .iter()
        .map(|name| json!({ "name": name, "description": "", "mode": "all", "native": true }))
        .collect();
    Json(Value::Array(arr))
}

// ── permissions / questions (PreToolUse hook) ───────────────────────

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

async fn internal_ask(State(engine): State<Engine>, body: Json<Value>) -> Json<Value> {
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

async fn permission_reply(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let reply = body.get("reply").and_then(|r| r.as_str()).unwrap_or("once").to_string();
    engine.resolve_pending(&id, PendingReply::Permission(reply));
    Json(json!({ "ok": true }))
}

async fn question_reply(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let answers: Vec<Vec<String>> =
        body.get("answers").and_then(|a| serde_json::from_value(a.clone()).ok()).unwrap_or_default();
    engine.resolve_pending(&id, PendingReply::Question(answers));
    Json(json!({ "ok": true }))
}

async fn question_reject(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    engine.resolve_pending(&id, PendingReply::Reject);
    Json(json!({ "ok": true }))
}

// ── SSE ─────────────────────────────────────────────────────────────

async fn event_stream(
    State(engine): State<Engine>,
    headers: HeaderMap,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let dir = dir_header(&headers);
    let mut rx = engine.subscribe();
    let stream = async_stream::stream! {
        yield Ok::<_, Infallible>(
            Event::default().data(json!({ "type": "server.connected", "properties": {} }).to_string()),
        );
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if dir.is_empty() || ev.directory.is_empty() || ev.directory == dir {
                        yield Ok::<_, Infallible>(Event::default().data(ev.data));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
