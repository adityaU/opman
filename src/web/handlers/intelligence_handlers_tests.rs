//! Generated tests for the computed-intelligence HTTP handlers.
//! Driven through the production router via `test_router` + `send_json`.

use super::*;
use crate::web::test_support::{send_json, test_router, test_server_state};
use axum::http::StatusCode;
use serde_json::json;

fn router() -> axum::Router {
    test_router(test_server_state())
}

// ── /api/inbox ──────────────────────────────────────────────────────

#[tokio::test]
async fn inbox_empty_body_ok() {
    let (status, body) = send_json(router(), "POST", "/api/inbox", Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("items").is_some());
}

#[tokio::test]
async fn inbox_with_inputs_ok() {
    let req = json!({
        "permissions": [],
        "questions": [],
        "watcher_status": null,
        "signals": []
    });
    let (status, _) = send_json(router(), "POST", "/api/inbox", Some(req)).await;
    assert_eq!(status, StatusCode::OK);
}

// ── /api/recommendations ────────────────────────────────────────────

#[tokio::test]
async fn recommendations_ok() {
    let (status, body) =
        send_json(router(), "POST", "/api/recommendations", Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("recommendations").is_some());
}

// ── /api/handoff/session ────────────────────────────────────────────

#[tokio::test]
async fn handoff_session_some_returns_200() {
    let req = json!({ "session_id": "sess-1234abcd" });
    let (status, body) = send_json(router(), "POST", "/api/handoff/session", Some(req)).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("title").is_some());
}

#[tokio::test]
async fn handoff_session_empty_id_returns_404() {
    let req = json!({ "session_id": "" });
    let (status, _) = send_json(router(), "POST", "/api/handoff/session", Some(req)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── /api/resume-briefing ────────────────────────────────────────────

#[tokio::test]
async fn resume_briefing_none_returns_204() {
    let (status, _) =
        send_json(router(), "POST", "/api/resume-briefing", Some(json!({}))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn resume_briefing_some_returns_200() {
    let req = json!({ "active_session_id": "sess-abcd" });
    let (status, body) = send_json(router(), "POST", "/api/resume-briefing", Some(req)).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("title").is_some());
}

// ── /api/daily-summary ──────────────────────────────────────────────

#[tokio::test]
async fn daily_summary_ok() {
    let req = json!({ "routine_id": "rtn-1" });
    let (status, body) = send_json(router(), "POST", "/api/daily-summary", Some(req)).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("summary").is_some());
}

// ── /api/signals ────────────────────────────────────────────────────

#[tokio::test]
async fn signals_list_ok() {
    let (status, body) = send_json(router(), "GET", "/api/signals", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("signals").is_some());
}

#[tokio::test]
async fn signals_add_returns_201_and_lists() {
    let router = router();
    let req = json!({ "kind": "note", "title": "hello", "body": "world" });
    let (status, body) =
        send_json(router.clone(), "POST", "/api/signals", Some(req)).await;
    assert_eq!(status, StatusCode::CREATED);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["title"], "hello");

    // The added signal is now listed.
    let (status, body) = send_json(router, "GET", "/api/signals", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["signals"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn signals_add_with_session_id() {
    let req = json!({ "kind": "alert", "title": "t", "body": "b", "session_id": "s9" });
    let (status, _) = send_json(router(), "POST", "/api/signals", Some(req)).await;
    assert_eq!(status, StatusCode::CREATED);
}

// ── /api/assistant-center/stats ─────────────────────────────────────

#[tokio::test]
async fn assistant_stats_ok() {
    let (status, _) = send_json(
        router(),
        "POST",
        "/api/assistant-center/stats",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// ── /api/workspace-templates ────────────────────────────────────────

#[tokio::test]
async fn workspace_templates_ok() {
    let (status, body) =
        send_json(router(), "GET", "/api/workspace-templates", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("templates").is_some());
}

// ── /api/memory/active ──────────────────────────────────────────────

#[tokio::test]
async fn active_memory_no_params_ok() {
    let (status, body) = send_json(router(), "GET", "/api/memory/active", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("memory").is_some());
}

#[tokio::test]
async fn active_memory_with_params_ok() {
    let (status, _) = send_json(
        router(),
        "GET",
        "/api/memory/active?project_index=0&session_id=s1",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
