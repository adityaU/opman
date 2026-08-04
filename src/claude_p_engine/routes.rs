//! opencode-compatible REST + SSE router for the `claude -p` engine. Handler groups:
//! sessions/messages here, provider/commands/agents in [`super::routes_meta`], and the
//! PreToolUse permission/question hook in [`super::routes_hook`].

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::dispatch::{dispatch_turn, extract_text};
use super::routes_hook::{internal_ask, permission_reply, question_reject, question_reply};
use super::routes_meta::{agent_list, command_list, provider};
use super::{process, session_info, ClaudePEngine};
use crate::claude_engine::{claude_cli, jsonl};

pub(super) type Engine = Arc<ClaudePEngine>;

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
            get(get_session)
                .patch(rename_session)
                .delete(delete_session),
        )
        .route(
            "/session/{id}/message",
            get(get_messages).post(send_message),
        )
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

// ── shared helpers ──────────────────────────────────────────────────

pub(super) fn dir_header(headers: &HeaderMap) -> String {
    headers
        .get("x-opencode-directory")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
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
    let parent = body
        .get("parentID")
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .to_string();
    let title = body
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("New session")
        .to_string();
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
    if let Some(model_id) = body
        .get("model")
        .and_then(|m| m.get("modelID"))
        .and_then(|s| s.as_str())
    {
        engine.set_model(&id, model_id);
    }
    if let Some(agent) = body.get("agent").and_then(|a| a.as_str()) {
        engine.set_agent(&id, agent);
    }
    if let Some(permission) = body.get("permission").and_then(|p| p.as_str()) {
        engine.set_permission_mode(&id, permission);
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
    let text = if args.is_empty() {
        format!("/{command}")
    } else {
        format!("/{command} {args}")
    };
    dispatch_turn(engine, id, text);
    Json(json!({ "ok": true }))
}

async fn abort(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    process::abort(engine, &id).await;
    Json(json!({ "ok": true }))
}

// ── messages / todos ────────────────────────────────────────────────

async fn get_messages(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    let Some(entry) = engine.get_session(&id) else {
        // A subagent child id the web UI backfills on reload.
        if let Some(path) = claude_cli::locate_subagent_jsonl(&id) {
            let parsed = jsonl::parse_file(&path, &id);
            return Json(Value::Array(
                parsed.messages.iter().map(|m| m.to_value()).collect(),
            ));
        }
        return Json(Value::Array(vec![]));
    };
    if entry.is_subagent {
        if let Some(path) = claude_cli::locate_subagent_jsonl(&entry.id) {
            let parsed = jsonl::parse_file(&path, &entry.id);
            return Json(Value::Array(
                parsed.messages.iter().map(|m| m.to_value()).collect(),
            ));
        }
        return Json(Value::Array(vec![]));
    }
    let Some(uuid) = entry.claude_uuid else {
        return Json(Value::Array(vec![]));
    };
    let Some(path) = claude_cli::locate_jsonl(&uuid) else {
        return Json(Value::Array(vec![]));
    };
    let mut parsed = jsonl::parse_file(&path, &id);
    jsonl::enrich_subagents(&mut parsed);
    jsonl::enrich_background_tasks(&mut parsed);
    Json(Value::Array(
        parsed.messages.iter().map(|m| m.to_value()).collect(),
    ))
}

async fn get_todos(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    let mut todos: Vec<Value> = vec![];
    if let Some(uuid) = engine.get_session(&id).and_then(|s| s.claude_uuid) {
        if let Some(path) = claude_cli::locate_jsonl(&uuid) {
            let parsed = jsonl::parse_file(&path, &id);
            for msg in &parsed.messages {
                for part in &msg.parts {
                    if part.get("tool").and_then(|t| t.as_str()) == Some("TodoWrite") {
                        if let Some(items) = part
                            .get("state")
                            .and_then(|s| s.get("input"))
                            .and_then(|i| i.get("todos"))
                            .and_then(|t| t.as_array())
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

#[cfg(test)]
#[path = "routes_endpoints_tests.rs"]
mod routes_endpoints_tests;

#[cfg(test)]
#[path = "routes_dispatch_tests.rs"]
mod routes_dispatch_tests;

#[cfg(test)]
#[path = "routes_transcript_tests.rs"]
mod routes_transcript_tests;

#[cfg(test)]
#[path = "routes_stream_tests.rs"]
mod routes_stream_tests;
