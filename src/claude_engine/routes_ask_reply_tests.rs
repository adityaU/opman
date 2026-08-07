//! Generated coverage tests for `routes.rs` — internal_ask, replies, SSE.
use super::*;
use crate::claude_engine::PendingReply;
use axum::extract::State;

fn engine() -> Engine {
    Arc::new(ClaudeEngine::new(None, crate::mcp_registry::RegistryHandle::default()))
}

async fn call_ask(e: &Engine, input: Value) -> Value {
    internal_ask(State(e.clone()), Json(input)).await.0
}

/// Await the next SSE-shaped event of `event_type` on `rx` and return its `properties.id`.
async fn wait_for_id(
    rx: &mut tokio::sync::broadcast::Receiver<super::super::EngineEvent>,
    event_type: &str,
) -> String {
    loop {
        let ev = rx.recv().await.unwrap();
        let v: Value = serde_json::from_str(&ev.data).unwrap();
        if v["type"] == event_type {
            return v["properties"]["id"].as_str().unwrap().to_string();
        }
    }
}

/// Resolve a pending request, retrying until the handler has actually registered it
/// (the handler emits the ask event *before* it inserts the pending receiver).
async fn resolve_retry(e: &Engine, id: &str, mut make: impl FnMut() -> PendingReply) {
    for _ in 0..10_000 {
        if e.resolve_pending(id, make()) {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("pending id was never registered");
}

// ── non-blocking internal_ask branches ─────────────────────────────

#[tokio::test]
async fn internal_ask_unknown_session_fails_open() {
    let e = engine();
    let out = call_ask(
        &e,
        json!({ "session_id": "unknown-uuid", "cwd": "/nowhere", "tool_name": "Bash",
                "tool_input": { "command": "ls" } }),
    )
    .await;
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "allow");
}

#[tokio::test]
async fn internal_ask_bypass_and_non_gated_allow() {
    let e = engine();
    let s = e.create_session("/d", "", "t"); // engine default mode = bypassPermissions
                                             // Gated tool but bypass mode → allow (resolved via newest-in-cwd fallback).
    let out = call_ask(
        &e,
        json!({ "session_id": "", "cwd": "/d", "tool_name": "Bash", "tool_input": {} }),
    )
    .await;
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "allow");

    // Non-gated tool with a prompting mode → allowed because it isn't gated.
    e.set_permission_mode(&s.id, "default");
    let out = call_ask(
        &e,
        json!({ "session_id": "", "cwd": "/d", "tool_name": "Read", "tool_input": {} }),
    )
    .await;
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "allow");
}

#[tokio::test]
async fn internal_ask_accept_edits_and_plan_and_always_allowed() {
    let e = engine();
    let s = e.create_session("/d", "", "t");

    e.set_permission_mode(&s.id, "acceptEdits");
    let out = call_ask(
        &e,
        json!({ "cwd": "/d", "tool_name": "Edit", "tool_input": {} }),
    )
    .await;
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "allow");

    e.set_permission_mode(&s.id, "plan");
    let out = call_ask(
        &e,
        json!({ "cwd": "/d", "tool_name": "Write", "tool_input": {} }),
    )
    .await;
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(out["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("Plan mode"));

    // Always-allowed tool short-circuits even in a prompting mode.
    e.set_permission_mode(&s.id, "default");
    e.add_allowed_tool(&s.id, "Bash");
    let out = call_ask(
        &e,
        json!({ "cwd": "/d", "tool_name": "Bash", "tool_input": {} }),
    )
    .await;
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "allow");
}

// ── blocking internal_ask branches (question + permission) ──────────

#[tokio::test]
async fn internal_ask_question_answered() {
    let e = engine();
    let s = e.create_session("/d", "", "t");
    let mut rx = e.subscribe();
    let e2 = e.clone();
    let sid = s.id.clone();
    let input = json!({
        "session_id": "", "cwd": "/d", "tool_name": "AskUserQuestion",
        "tool_input": { "questions": [ { "question": "Pick?", "options": [ { "label": "A" } ] } ] },
    });
    let _ = sid;
    let h = tokio::spawn(async move { call_ask(&e2, input).await });
    let id = wait_for_id(&mut rx, "question.asked").await;
    resolve_retry(&e, &id, || {
        PendingReply::Question(vec![vec!["A".to_string()]])
    })
    .await;
    let out = h.await.unwrap();
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(out["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("USER ANSWER"));
}

#[tokio::test]
async fn internal_ask_question_rejected() {
    let e = engine();
    e.create_session("/d", "", "t");
    let mut rx = e.subscribe();
    let e2 = e.clone();
    let input = json!({
        "session_id": "", "cwd": "/d", "tool_name": "AskUserQuestion",
        "tool_input": { "questions": [ { "question": "Q?" } ] },
    });
    let h = tokio::spawn(async move { call_ask(&e2, input).await });
    let id = wait_for_id(&mut rx, "question.asked").await;
    resolve_retry(&e, &id, || PendingReply::Reject).await;
    let out = h.await.unwrap();
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(out["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("dismissed"));
}

#[tokio::test]
async fn internal_ask_permission_always_reject_once() {
    // "always" → allow + remembers the tool.
    let e = engine();
    let s = e.create_session("/d", "", "t");
    e.set_permission_mode(&s.id, "default");
    let mut rx = e.subscribe();
    let e2 = e.clone();
    let input = json!({ "cwd": "/d", "tool_name": "Bash", "tool_input": { "command": "ls" } });
    let h = tokio::spawn(async move { call_ask(&e2, input).await });
    let id = wait_for_id(&mut rx, "permission.asked").await;
    resolve_retry(&e, &id, || PendingReply::Permission("always".to_string())).await;
    let out = h.await.unwrap();
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "allow");
    assert!(e
        .get_session(&s.id)
        .unwrap()
        .allowed_tools
        .contains(&"Bash".to_string()));

    // "reject" → deny.
    let e2 = e.clone();
    let mut rx = e.subscribe();
    // Use a fresh gated tool that is not always-allowed.
    let input = json!({ "cwd": "/d", "tool_name": "Write", "tool_input": { "file_path": "/x" } });
    let h = tokio::spawn(async move { call_ask(&e2, input).await });
    let id = wait_for_id(&mut rx, "permission.asked").await;
    resolve_retry(&e, &id, || PendingReply::Permission("reject".to_string())).await;
    let out = h.await.unwrap();
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");

    // "once" (any other reply) → allow.
    let e2 = e.clone();
    let mut rx = e.subscribe();
    let input = json!({ "cwd": "/d", "tool_name": "Edit", "tool_input": { "path": "/y" } });
    let h = tokio::spawn(async move { call_ask(&e2, input).await });
    let id = wait_for_id(&mut rx, "permission.asked").await;
    resolve_retry(&e, &id, || PendingReply::Permission("once".to_string())).await;
    let out = h.await.unwrap();
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "allow");
}

#[tokio::test]
async fn internal_ask_permission_reject_reply_kind() {
    // Resolving with a non-Permission reply hits the `_` timeout/reject arm.
    let e = engine();
    let s = e.create_session("/d", "", "t");
    e.set_permission_mode(&s.id, "default");
    let mut rx = e.subscribe();
    let e2 = e.clone();
    let input = json!({ "cwd": "/d", "tool_name": "Bash", "tool_input": { "command": "rm" } });
    let h = tokio::spawn(async move { call_ask(&e2, input).await });
    let id = wait_for_id(&mut rx, "permission.asked").await;
    resolve_retry(&e, &id, || PendingReply::Reject).await;
    let out = h.await.unwrap();
    assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(out["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("not answered"));
}

// ── emit_resolved + reply endpoints ─────────────────────────────────

#[tokio::test]
async fn emit_resolved_broadcasts() {
    let e = engine();
    let mut rx = e.subscribe();
    emit_resolved(&e, "/d", "req1", "ses1", "permission.replied");
    let ev = rx.recv().await.unwrap();
    let v: Value = serde_json::from_str(&ev.data).unwrap();
    assert_eq!(v["type"], "permission.replied");
    assert_eq!(v["properties"]["id"], "req1");
    assert_eq!(v["properties"]["requestID"], "req1");
    assert_eq!(v["properties"]["sessionID"], "ses1");
}

async fn send(router: Router, method: &str, uri: &str, body: Option<Value>) -> Value {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(match body {
            Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
            None => Body::empty(),
        })
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn reply_endpoints_are_idempotent_on_unknown_ids() {
    let r = router(engine());
    // Unknown ids stay 200 (idempotent) but report ok:false, so the runner
    // registry's fan-out knows this engine did not own the request.
    let v = send(
        r.clone(),
        "POST",
        "/permission/nope/reply",
        Some(json!({ "reply": "always" })),
    )
    .await;
    assert_eq!(v["ok"], false);
    // Missing reply defaults to "once".
    let v = send(
        r.clone(),
        "POST",
        "/permission/nope2/reply",
        Some(json!({})),
    )
    .await;
    assert_eq!(v["ok"], false);
    let v = send(
        r.clone(),
        "POST",
        "/question/nope/reply",
        Some(json!({ "answers": [["A"]] })),
    )
    .await;
    assert_eq!(v["ok"], false);
    // Invalid answers shape → default empty, still a clean 200.
    let v = send(
        r.clone(),
        "POST",
        "/question/nope/reply",
        Some(json!({ "answers": "bad" })),
    )
    .await;
    assert_eq!(v["ok"], false);
    let v = send(r, "POST", "/question/nope/reject", None).await;
    assert_eq!(v["ok"], false);
}

#[tokio::test]
async fn permission_reply_resolves_pending() {
    let e = engine();
    let r = router(e.clone());
    let rx = e.register_pending("perm_known");
    let v = send(
        r,
        "POST",
        "/permission/perm_known/reply",
        Some(json!({ "reply": "always" })),
    )
    .await;
    assert_eq!(v["ok"], true);
    let got = rx.await.unwrap();
    assert!(matches!(got, PendingReply::Permission(s) if s == "always"));
}

#[tokio::test]
async fn question_reply_resolves_pending_with_answers() {
    let e = engine();
    let r = router(e.clone());
    let rx = e.register_pending("q_known");
    let v = send(
        r,
        "POST",
        "/question/q_known/reply",
        Some(json!({ "answers": [["Yes"], ["No", "Maybe"]] })),
    )
    .await;
    assert_eq!(v["ok"], true);
    let got = rx.await.unwrap();
    match got {
        PendingReply::Question(a) => {
            assert_eq!(
                a,
                vec![
                    vec!["Yes".to_string()],
                    vec!["No".to_string(), "Maybe".to_string()]
                ]
            );
        }
        _ => panic!("expected Question"),
    }
}

#[tokio::test]
async fn question_reject_resolves_pending() {
    let e = engine();
    let r = router(e.clone());
    let rx = e.register_pending("q_rej");
    let v = send(r, "POST", "/question/q_rej/reject", None).await;
    assert_eq!(v["ok"], true);
    assert!(matches!(rx.await.unwrap(), PendingReply::Reject));
}

// ── SSE stream body ─────────────────────────────────────────────────

#[tokio::test]
async fn event_stream_body_emits_connected_then_matching_events() {
    use futures::StreamExt;
    use std::time::Duration;

    let e = engine();
    let stream = event_stream_body(e.clone(), "/d".to_string());
    tokio::pin!(stream);

    // First item is always the synthetic server.connected event.
    let first = tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .unwrap();
    assert!(first.is_some());

    // A matching-directory event is delivered.
    e.emit("/d", "custom.event", json!({ "x": 1 }));
    let second = tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .unwrap();
    assert!(second.is_some());

    // An unscoped (empty-directory) event is also delivered to a scoped subscriber.
    e.emit("", "broadcast.event", json!({}));
    let third = tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .unwrap();
    assert!(third.is_some());
}

#[tokio::test]
async fn event_stream_handler_constructs_sse() {
    // Drives the `event_stream` wrapper (dir_header + Sse::new). The stream body is not
    // polled here (covered separately by event_stream_body tests).
    let e = engine();
    let mut headers = HeaderMap::new();
    headers.insert("x-opencode-directory", "/d".parse().unwrap());
    let _sse = event_stream(State(e), headers).await;
}

#[tokio::test]
async fn event_stream_body_unscoped_subscriber_gets_everything() {
    use futures::StreamExt;
    use std::time::Duration;

    let e = engine();
    let stream = event_stream_body(e.clone(), String::new());
    tokio::pin!(stream);
    let _connected = tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .unwrap();
    e.emit("/some/other/dir", "x", json!({}));
    let ev = tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .unwrap();
    assert!(ev.is_some());
}
