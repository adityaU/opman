//! Generated tests for `handle_web_sse_event` (part 1): parsing, stats,
//! session lifecycle, and status transitions.

use super::*;
use crate::web::types::WebEvent;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;
use tokio::sync::broadcast::Receiver;

pub(super) fn handle_for(proj: &str) -> WebStateHandle {
    WebStateHandle::new_test_with_projects(vec![("p".to_string(), PathBuf::from(proj))])
}

pub(super) fn drain(rx: &mut Receiver<WebEvent>) -> Vec<WebEvent> {
    let mut out = Vec::new();
    while let Ok(e) = rx.try_recv() {
        out.push(e);
    }
    out
}

#[tokio::test]
async fn unparseable_data_is_ignored() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(&h, "not-json", "/proj").await;
    handle_web_sse_event(&h, r#"{"no":"type"}"#, "/proj").await;
    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn unknown_event_type_is_ignored() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(&h, r#"{"type":"whatever.else","properties":{}}"#, "/proj").await;
    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn message_updated_envelope_emits_stats() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    let data = r#"{"directory":"/proj","payload":{"type":"message.updated","properties":{
        "info":{"sessionID":"s1","cost":0.5,
            "tokens":{"input":10,"output":5,"reasoning":2,"cache":{"read":1,"write":3}}}}}}"#;
    handle_web_sse_event(&h, data, "/proj").await;
    let evs = drain(&mut rx);
    assert!(evs
        .iter()
        .any(|e| matches!(e, WebEvent::StatsUpdated(s) if s.session_id == "s1")));
}

#[tokio::test]
async fn message_updated_bare_event_also_works() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    let data = r#"{"type":"message.updated","properties":{"info":{"sessionID":"s2","cost":0.1,
        "tokens":{"input":1,"output":1}}}}"#;
    handle_web_sse_event(&h, data, "/proj").await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::StatsUpdated(_))));
}

#[tokio::test]
async fn message_updated_without_info_or_empty_session_is_noop() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(&h, r#"{"type":"message.updated","properties":{}}"#, "/proj").await;
    handle_web_sse_event(
        &h,
        r#"{"type":"message.updated","properties":{"info":{"sessionID":""}}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn session_created_adds_and_activates_root() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    let data = r#"{"type":"session.created","properties":{"info":{
        "id":"root1","title":"T","parentID":"","directory":"/proj","time":{"created":1,"updated":2}}}}"#;
    handle_web_sse_event(&h, data, "/proj").await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::StateChanged)));
    // Duplicate create must not error and still emits StateChanged.
    handle_web_sse_event(&h, data, "/proj").await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::StateChanged)));
}

#[tokio::test]
async fn session_created_child_tracks_parent() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    // Root first.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.created","properties":{"info":{"id":"root","parentID":"","directory":"/proj"}}}"#,
        "/proj",
    )
    .await;
    let _ = drain(&mut rx);
    // Child with a parentID -> recorded in session_children (empty-dir match branch).
    handle_web_sse_event(
        &h,
        r#"{"type":"session.created","properties":{"info":{"id":"child","parentID":"root","directory":""}}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::StateChanged)));
}

#[tokio::test]
async fn session_updated_existing_and_missing() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"session.created","properties":{"info":{"id":"u1","directory":"/proj"}}}"#,
        "/proj",
    )
    .await;
    let _ = drain(&mut rx);
    // Update the existing session.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.updated","properties":{"info":{"id":"u1","title":"new","directory":"/proj"}}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::StateChanged)));
    // Update an unknown session: still emits StateChanged.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.updated","properties":{"info":{"id":"ghost","directory":"/proj"}}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::StateChanged)));
}

#[tokio::test]
async fn session_deleted_cleans_up() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"session.created","properties":{"info":{"id":"d1","directory":"/proj"}}}"#,
        "/proj",
    )
    .await;
    let _ = drain(&mut rx);
    handle_web_sse_event(
        &h,
        r#"{"type":"session.deleted","properties":{"sessionID":"d1"}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::StateChanged)));
}

#[tokio::test]
async fn session_status_busy_then_idle() {
    let h = handle_for("/proj");
    // Register the session as active so idle does NOT count as unseen.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.created","properties":{"info":{"id":"b1","parentID":"","directory":"/proj"}}}"#,
        "/proj",
    )
    .await;
    let mut rx = h.subscribe_events();

    handle_web_sse_event(
        &h,
        r#"{"type":"session.status","properties":{"sessionID":"b1","status":{"type":"busy"}}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::SessionBusy { session_id } if session_id == "b1")));

    handle_web_sse_event(
        &h,
        r#"{"type":"session.status","properties":{"sessionID":"b1","status":{"type":"idle"}}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::SessionIdle { session_id } if session_id == "b1")));
}

#[tokio::test]
async fn session_status_retry_emits_busy() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"session.status","properties":{"sessionID":"r1","status":{"type":"retry"}}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::SessionBusy { .. })));
}

#[tokio::test]
async fn session_status_idle_unknown_session_counts_unseen() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    // Idle for a session not in any project -> treated as root, not active -> unseen.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.status","properties":{"sessionID":"x9","status":{"type":"idle"}}}"#,
        "/proj",
    )
    .await;
    assert!(drain(&mut rx).iter().any(|e| matches!(e, WebEvent::SessionUnseen { session_id, count } if session_id == "x9" && *count == 1)));
}

#[tokio::test]
async fn session_status_idle_subagent_skips_unseen() {
    let h = handle_for("/proj");
    // Create a subagent (parentID set) so idle skips the unseen path.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.created","properties":{"info":{"id":"sub","parentID":"root","directory":"/proj"}}}"#,
        "/proj",
    )
    .await;
    let mut rx = h.subscribe_events();
    handle_web_sse_event(
        &h,
        r#"{"type":"session.status","properties":{"sessionID":"sub","status":{"type":"idle"}}}"#,
        "/proj",
    )
    .await;
    // No SessionUnseen for a subagent.
    assert!(!drain(&mut rx)
        .iter()
        .any(|e| matches!(e, WebEvent::SessionUnseen { .. })));
}

#[tokio::test]
async fn session_status_unknown_type_records_activity_only() {
    let h = handle_for("/proj");
    let mut rx = h.subscribe_events();
    // A status type outside busy/idle/retry falls through both matches.
    handle_web_sse_event(
        &h,
        r#"{"type":"session.status","properties":{"sessionID":"c1","status":{"type":"compacting"}}}"#,
        "/proj",
    )
    .await;
    // No busy/idle/unseen events emitted for an unhandled status type.
    let evs = drain(&mut rx);
    assert!(!evs.iter().any(|e| matches!(
        e,
        WebEvent::SessionBusy { .. }
            | WebEvent::SessionIdle { .. }
            | WebEvent::SessionUnseen { .. }
    )));
}
