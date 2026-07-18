//! Router tests for message dispatch, transcript-backed reads, and abort.
use super::*;
use crate::claude_p_engine::ClaudePEngine;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

async fn send(router: Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Vec<u8>) {
    let b = Request::builder().method(method).uri(uri).header("content-type", "application/json");
    let req = match body {
        Some(v) => b.body(Body::from(serde_json::to_vec(&v).unwrap())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, bytes)
}

fn as_json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[tokio::test]
async fn send_message_sets_model_and_agent() {
    let e = engine();
    let s = e.create_session("/proj", "", "A");
    let body = json!({
        "model": { "modelID": "m1" },
        "agent": "Plan",
        "parts": [ { "type": "text", "text": "" } ]
    });
    let (st, out) =
        send(router(e.clone()), "POST", &format!("/session/{}/message", s.id), Some(body)).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&out)["ok"], true);
    let sess = e.get_session(&s.id).unwrap();
    assert_eq!(sess.model.as_deref(), Some("m1"));
    assert_eq!(sess.agent.as_deref(), Some("Plan"));
    // Empty text → no process spawned, session idle.
    assert!(!sess.busy);
}

#[tokio::test]
async fn prompt_async_route_dispatches() {
    let e = engine();
    let s = e.create_session("/proj", "", "A");
    // A control command → consumed, no child spawned.
    let body = json!({ "parts": [ { "type": "text", "text": "/permission-mode plan" } ] });
    let (st, out) =
        send(router(e.clone()), "POST", &format!("/session/{}/prompt_async", s.id), Some(body)).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&out)["ok"], true);
    assert_eq!(e.get_session(&s.id).unwrap().permission_mode.as_deref(), Some("plan"));
}

#[tokio::test]
async fn session_command_endpoint_with_and_without_args() {
    let e = engine();
    let s = e.create_session("/proj", "", "A");
    let (st, out) = send(
        router(e.clone()),
        "POST",
        &format!("/session/{}/command", s.id),
        Some(json!({ "command": "permission-mode", "arguments": "plan" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&out)["ok"], true);
    assert_eq!(e.get_session(&s.id).unwrap().permission_mode.as_deref(), Some("plan"));

    // No arguments → "/permission-mode" alone (unknown mode toast, still consumed).
    let (_st, out2) = send(
        router(e.clone()),
        "POST",
        &format!("/session/{}/command", s.id),
        Some(json!({ "command": "permission-mode" })),
    )
    .await;
    assert_eq!(as_json(&out2)["ok"], true);
}

#[tokio::test]
async fn abort_endpoint() {
    let e = engine();
    let s = e.create_session("/proj", "", "A");
    e.set_busy(&s.id, true);
    let (st, out) = send(router(e.clone()), "POST", &format!("/session/{}/abort", s.id), Some(json!({}))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&out)["ok"], true);
    assert!(!e.get_session(&s.id).unwrap().busy);
}

#[tokio::test]
async fn get_messages_unknown_session_empty() {
    let e = engine();
    // Random id: no session, no subagent transcript → empty array.
    let (st, out) = send(router(e), "GET", "/session/does-not-exist/message", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&out), json!([]));
}

#[tokio::test]
async fn get_messages_no_uuid_empty() {
    let e = engine();
    let s = e.create_session("/proj", "", "A");
    let (_st, out) = send(router(e), "GET", &format!("/session/{}/message", s.id), None).await;
    assert_eq!(as_json(&out), json!([]));
}

#[tokio::test]
async fn get_messages_uuid_without_transcript_empty() {
    let e = engine();
    let s = e.create_session("/proj", "", "A");
    e.set_claude_uuid(&s.id, "opman-nonexistent-uuid-zzz");
    let (_st, out) = send(router(e), "GET", &format!("/session/{}/message", s.id), None).await;
    assert_eq!(as_json(&out), json!([]));
}

#[tokio::test]
async fn get_messages_subagent_without_transcript_empty() {
    let e = engine();
    let parent = e.create_session("/proj", "", "A");
    e.ensure_subagent_session(&parent.id, "agent-xyz", "", "/proj");
    let (_st, out) = send(router(e), "GET", "/session/agent-xyz/message", None).await;
    assert_eq!(as_json(&out), json!([]));
}

#[tokio::test]
async fn get_todos_empty_paths() {
    let e = engine();
    let s = e.create_session("/proj", "", "A");
    // No uuid → empty.
    let (st, out) = send(router(e.clone()), "GET", &format!("/session/{}/todo", s.id), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&out), json!([]));
    // Uuid set but no transcript on disk → still empty.
    e.set_claude_uuid(&s.id, "opman-nonexistent-uuid-todo");
    let (_st, out2) = send(router(e), "GET", &format!("/session/{}/todo", s.id), None).await;
    assert_eq!(as_json(&out2), json!([]));
}
