//! Wave-3 tests driving the extracted single-iteration poller helpers
//! (`poll_session_status_once`, `fetch_provider_limits_once`) against a real
//! mock "opencode" upstream. These cover the fetch-success/parse/transition
//! branches that the forever-looping `spawn_*` tasks never exercise in a unit
//! test (they sleep 3s and hit a dead base_url).

use super::*;
use crate::app::BackgroundEvent;
use crate::web::test_support::start_mock_upstream;
use axum::routing::get;
use std::collections::HashSet;
use tokio::sync::mpsc;

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// ── poll_session_status_once ────────────────────────────────────────

#[tokio::test]
async fn poll_session_once_emits_newly_busy() {
    // /session/status reports s1 busy, s2 idle (idle absent from active set).
    let router = axum::Router::new().route(
        "/session/status",
        get(|| async {
            axum::Json(serde_json::json!({
                "s1": { "type": "busy" },
                "s2": { "type": "idle" }
            }))
        }),
    );
    let base = start_mock_upstream(router).await;
    let client = crate::api::ApiClient::new();
    let (tx, mut rx) = mpsc::unbounded_channel::<BackgroundEvent>();

    let known = HashSet::new();
    let active = poll_session_status_once(&client, &base, "/proj", &known, &tx, 3)
        .await
        .expect("fetch succeeds");
    assert_eq!(active, set(&["s1"]));

    // Exactly one SseSessionBusy{s1} should have been emitted.
    let ev = rx.try_recv().expect("one event");
    assert!(matches!(ev, BackgroundEvent::SseSessionBusy { session_id } if session_id == "s1"));
    assert!(rx.try_recv().is_err(), "no further events");
}

#[tokio::test]
async fn poll_session_once_emits_newly_idle() {
    // Server now reports nothing active; known_active still holds s1 → idle.
    let router = axum::Router::new().route(
        "/session/status",
        get(|| async { axum::Json(serde_json::json!({})) }),
    );
    let base = start_mock_upstream(router).await;
    let client = crate::api::ApiClient::new();
    let (tx, mut rx) = mpsc::unbounded_channel::<BackgroundEvent>();

    let known = set(&["s1"]);
    let active = poll_session_status_once(&client, &base, "/proj", &known, &tx, 9)
        .await
        .expect("fetch succeeds");
    assert!(active.is_empty());

    let ev = rx.try_recv().expect("one event");
    match ev {
        BackgroundEvent::SseSessionIdle {
            project_idx,
            session_id,
        } => {
            assert_eq!(project_idx, 9);
            assert_eq!(session_id, "s1");
        }
        _ => panic!("expected SseSessionIdle, got a different event"),
    }
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn poll_session_once_no_change_emits_nothing() {
    let router = axum::Router::new().route(
        "/session/status",
        get(|| async { axum::Json(serde_json::json!({ "s1": { "type": "busy" } })) }),
    );
    let base = start_mock_upstream(router).await;
    let client = crate::api::ApiClient::new();
    let (tx, mut rx) = mpsc::unbounded_channel::<BackgroundEvent>();

    let known = set(&["s1"]);
    let active = poll_session_status_once(&client, &base, "/proj", &known, &tx, 0)
        .await
        .unwrap();
    assert_eq!(active, set(&["s1"]));
    assert!(rx.try_recv().is_err(), "no transitions when unchanged");
}

#[tokio::test]
async fn poll_session_once_fetch_error_returns_none() {
    // Nothing listens on this port → fetch_session_status errors → None.
    let client = crate::api::ApiClient::new();
    let (tx, _rx) = mpsc::unbounded_channel::<BackgroundEvent>();
    let known = HashSet::new();
    let res = poll_session_status_once(&client, "http://127.0.0.1:1", "/proj", &known, &tx, 0).await;
    assert!(res.is_none());
}

// ── fetch_provider_limits_once ──────────────────────────────────────

#[tokio::test]
async fn fetch_provider_once_returns_max_context() {
    let router = axum::Router::new().route(
        "/provider",
        get(|| async {
            axum::Json(serde_json::json!([
                { "models": { "m1": { "limit": { "context": 100_000 } } } },
                { "models": { "m2": { "limit": { "context": 500_000 } } } }
            ]))
        }),
    );
    let base = start_mock_upstream(router).await;
    let client = reqwest::Client::new();
    let got = fetch_provider_limits_once(&client, &base, "/proj").await;
    assert_eq!(got, Some(500_000));
}

#[tokio::test]
async fn fetch_provider_once_zero_context_returns_none() {
    // Well-formed body but no positive context window → None (caller retries).
    let router = axum::Router::new().route(
        "/provider",
        get(|| async { axum::Json(serde_json::json!([{ "models": {} }])) }),
    );
    let base = start_mock_upstream(router).await;
    let client = reqwest::Client::new();
    assert_eq!(fetch_provider_limits_once(&client, &base, "/proj").await, None);
}

#[tokio::test]
async fn fetch_provider_once_non_success_status_returns_none() {
    let router = axum::Router::new().route(
        "/provider",
        get(|| async { axum::http::StatusCode::INTERNAL_SERVER_ERROR }),
    );
    let base = start_mock_upstream(router).await;
    let client = reqwest::Client::new();
    assert_eq!(fetch_provider_limits_once(&client, &base, "/proj").await, None);
}

#[tokio::test]
async fn fetch_provider_once_malformed_body_returns_none() {
    // 200 OK but the body is not valid JSON → r.json() errors → None.
    let router = axum::Router::new().route(
        "/provider",
        get(|| async { "not json{" }),
    );
    let base = start_mock_upstream(router).await;
    let client = reqwest::Client::new();
    assert_eq!(fetch_provider_limits_once(&client, &base, "/proj").await, None);
}

#[tokio::test]
async fn fetch_provider_once_connection_error_returns_none() {
    let client = reqwest::Client::new();
    assert_eq!(
        fetch_provider_limits_once(&client, "http://127.0.0.1:1", "/proj").await,
        None
    );
}
