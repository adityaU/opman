//! opencode-compatible REST + SSE router for an ACP agent.
//!
//! The contract is unchanged from the engine this replaces, so the web UI and the runner
//! registry need no knowledge of ACP. What is gone is the `/internal/ask` hook endpoint:
//! permission is a protocol round-trip now, not an HTTP callback from a subprocess.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::routes_meta::{agent_list, command_list, provider};
use super::routes_turn::{
    abort, get_messages, get_todos, permission_reply, question_reject, question_reply,
    send_message, session_command,
};
use super::{session_info, AcpEngine};


pub(super) type Engine = Arc<AcpEngine>;

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
        .with_state(engine)
}

pub(super) fn dir_header(headers: &HeaderMap) -> String {
    headers
        .get("x-opencode-directory")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

// ── basic handlers ──────────────────────────────────────────────────

async fn info(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    Json(json!({
        "directory": dir_header(&headers),
        "version": env!("CARGO_PKG_VERSION"),
        "agent": engine.id,
    }))
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
    let sessions: Vec<Value> = engine.list_for_dir(&dir).iter().map(session_info).collect();
    Json(Value::Array(sessions))
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
        .and_then(Value::as_str)
        .unwrap_or_default();
    let title = body
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("New session");
    Json(session_info(&engine.create_session(&dir, parent, title)))
}

async fn get_session(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    match engine.get_session(&id) {
        Some(entry) => Json(session_info(&entry)),
        None => Json(json!({ "id": id })),
    }
}

async fn rename_session(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    if let Some(title) = body.get("title").and_then(Value::as_str) {
        engine.rename_session(&id, title);
    }
    match engine.get_session(&id) {
        Some(entry) => Json(session_info(&entry)),
        None => Json(json!({ "id": id })),
    }
}

async fn delete_session(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    engine.delete_session(&id).await;
    Json(json!({ "ok": true }))
}

async fn session_status(State(engine): State<Engine>) -> Json<Value> {
    let mut status = serde_json::Map::new();
    for (id, busy) in engine.busy_map() {
        if busy {
            status.insert(id, json!({ "type": "busy" }));
        }
    }
    Json(Value::Object(status))
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
#[path = "routes_tests.rs"]
mod routes_tests;
