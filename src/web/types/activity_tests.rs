use super::*;
use serde_json::{json, Value};

#[test]
fn activity_event_payload_serializes_with_detail() {
    let ev = ActivityEventPayload {
        session_id: "s1".into(),
        kind: "file_edit".into(),
        summary: "edited foo.rs".into(),
        detail: Some("foo.rs".into()),
        timestamp: "2026-07-17T00:00:00Z".into(),
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["session_id"], "s1");
    assert_eq!(v["kind"], "file_edit");
    assert_eq!(v["summary"], "edited foo.rs");
    assert_eq!(v["detail"], "foo.rs");
    assert_eq!(v["timestamp"], "2026-07-17T00:00:00Z");
}

#[test]
fn activity_event_payload_omits_none_detail() {
    let ev = ActivityEventPayload {
        session_id: "s1".into(),
        kind: "status".into(),
        summary: "idle".into(),
        detail: None,
        timestamp: "t".into(),
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert!(v.get("detail").is_none(), "detail must be skipped when None");
    // Clone/Debug coverage.
    let c = ev.clone();
    assert_eq!(c.session_id, "s1");
    let _ = format!("{ev:?}");
}

#[test]
fn activity_feed_response_serializes() {
    let resp = ActivityFeedResponse {
        session_id: "sess".into(),
        events: vec![ActivityEventPayload {
            session_id: "sess".into(),
            kind: "tool_call".into(),
            summary: "ran tool".into(),
            detail: None,
            timestamp: "t".into(),
        }],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["session_id"], "sess");
    assert_eq!(v["events"].as_array().unwrap().len(), 1);
    let _ = format!("{resp:?}");
    let _ = resp.clone();
}

#[test]
fn activity_feed_response_empty_events() {
    let resp = ActivityFeedResponse {
        session_id: "x".into(),
        events: vec![],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["events"], json!([]));
}

#[test]
fn search_result_entry_serializes() {
    let e = SearchResultEntry {
        session_id: "s".into(),
        session_title: "Title".into(),
        project_name: "proj".into(),
        message_id: "m1".into(),
        role: "user".into(),
        snippet: "hello world".into(),
        timestamp: 1_700_000_000,
    };
    let v: Value = serde_json::to_value(&e).unwrap();
    assert_eq!(v["session_id"], "s");
    assert_eq!(v["session_title"], "Title");
    assert_eq!(v["project_name"], "proj");
    assert_eq!(v["message_id"], "m1");
    assert_eq!(v["role"], "user");
    assert_eq!(v["snippet"], "hello world");
    assert_eq!(v["timestamp"], 1_700_000_000u64);
    let _ = format!("{e:?}");
    let _ = e.clone();
}

#[test]
fn search_response_serializes() {
    let resp = SearchResponse {
        query: "foo".into(),
        results: vec![SearchResultEntry {
            session_id: "s".into(),
            session_title: "t".into(),
            project_name: "p".into(),
            message_id: "m".into(),
            role: "assistant".into(),
            snippet: "match".into(),
            timestamp: 1,
        }],
        total: 1,
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["query"], "foo");
    assert_eq!(v["total"], 1);
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
    let _ = format!("{resp:?}");
    let _ = resp.clone();
}
