//! Blocking `internal_ask` paths: emit a request, capture its id from the event
//! stream, then resolve the pending reply concurrently.
use super::*;
use crate::claude_engine::EngineEvent;
use crate::claude_p_engine::ClaudePEngine;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

async fn wait_event_id(rx: &mut Receiver<EngineEvent>, want: &str) -> String {
    loop {
        let ev = rx.recv().await.unwrap();
        let v: Value = serde_json::from_str(&ev.data).unwrap();
        if v["type"] == want {
            return v["properties"]["id"].as_str().unwrap().to_string();
        }
    }
}

/// Resolve as soon as the handler has registered its pending receiver (there is a
/// small window between the emit we observe and the `register_pending` call).
async fn resolve_when_ready(e: &Arc<ClaudePEngine>, id: &str, make: impl Fn() -> PendingReply) {
    for _ in 0..100_000 {
        if e.resolve_pending(id, make()) {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("pending {id} was never registered");
}

#[tokio::test]
async fn permission_ask_always_grants_and_persists() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "uu");
    e.set_permission_mode(&s.id, "default");
    let mut rx = e.subscribe();

    let e2 = e.clone();
    let handle = tokio::spawn(async move {
        let body = json!({ "session_id": "uu", "cwd": "d", "tool_name": "Bash", "tool_input": { "command": "ls" } });
        internal_ask(State(e2), Json(body)).await
    });

    let id = wait_event_id(&mut rx, "permission.asked").await;
    resolve_when_ready(&e, &id, || PendingReply::Permission("always".into())).await;

    let Json(resp) = handle.await.unwrap();
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "allow");
    assert!(e.is_always_allowed(&s.id, "Bash"));
}

#[tokio::test]
async fn permission_ask_reject_denies() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "uu");
    e.set_permission_mode(&s.id, "default");
    let mut rx = e.subscribe();

    let e2 = e.clone();
    let handle = tokio::spawn(async move {
        let body = json!({ "session_id": "uu", "cwd": "d", "tool_name": "Bash", "tool_input": {} });
        internal_ask(State(e2), Json(body)).await
    });
    let id = wait_event_id(&mut rx, "permission.asked").await;
    resolve_when_ready(&e, &id, || PendingReply::Permission("reject".into())).await;
    let Json(resp) = handle.await.unwrap();
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[tokio::test]
async fn permission_ask_once_allows() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "uu");
    e.set_permission_mode(&s.id, "default");
    let mut rx = e.subscribe();

    let e2 = e.clone();
    let handle = tokio::spawn(async move {
        let body = json!({ "session_id": "uu", "cwd": "d", "tool_name": "Bash", "tool_input": {} });
        internal_ask(State(e2), Json(body)).await
    });
    let id = wait_event_id(&mut rx, "permission.asked").await;
    resolve_when_ready(&e, &id, || PendingReply::Permission("once".into())).await;
    let Json(resp) = handle.await.unwrap();
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "allow");
    // "once" does not persist the tool.
    assert!(!e.is_always_allowed(&s.id, "Bash"));
}

#[tokio::test]
async fn permission_ask_wrong_variant_denies() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "uu");
    e.set_permission_mode(&s.id, "default");
    let mut rx = e.subscribe();

    let e2 = e.clone();
    let handle = tokio::spawn(async move {
        let body = json!({ "session_id": "uu", "cwd": "d", "tool_name": "Bash", "tool_input": {} });
        internal_ask(State(e2), Json(body)).await
    });
    let id = wait_event_id(&mut rx, "permission.asked").await;
    // A non-Permission reply hits the fallback (`_`) arm → deny "not answered".
    resolve_when_ready(&e, &id, || PendingReply::Reject).await;
    let Json(resp) = handle.await.unwrap();
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[tokio::test]
async fn ask_user_question_answered() {
    let e = engine();
    let _s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();

    let e2 = e.clone();
    let handle = tokio::spawn(async move {
        let body = json!({
            "session_id": "", "cwd": "d", "tool_name": "AskUserQuestion",
            "tool_input": { "questions": [ { "question": "Pick" } ] }
        });
        internal_ask(State(e2), Json(body)).await
    });
    let id = wait_event_id(&mut rx, "question.asked").await;
    resolve_when_ready(&e, &id, || PendingReply::Question(vec![vec!["red".into()]])).await;
    let Json(resp) = handle.await.unwrap();
    // Answers are injected via a deny reason carrying the user's answer text.
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "deny");
    let reason = resp["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap();
    assert!(reason.contains("red"));
    assert!(reason.contains("[USER ANSWER]"));
}

#[tokio::test]
async fn ask_user_question_dismissed() {
    let e = engine();
    let _s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();

    let e2 = e.clone();
    let handle = tokio::spawn(async move {
        let body = json!({
            "session_id": "", "cwd": "d", "tool_name": "AskUserQuestion",
            "tool_input": { "questions": [ { "question": "Pick" } ] }
        });
        internal_ask(State(e2), Json(body)).await
    });
    let id = wait_event_id(&mut rx, "question.asked").await;
    // A Reject (or any non-Question) reply → dismissed fallback.
    resolve_when_ready(&e, &id, || PendingReply::Reject).await;
    let Json(resp) = handle.await.unwrap();
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "deny");
    let reason = resp["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap();
    assert!(reason.contains("dismissed"));
}
