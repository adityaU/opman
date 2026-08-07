//! Single-iteration tests for the session/SSE/persist poller bodies extracted
//! from the infinite `spawn_*` loops. Fetch-success branches are driven against
//! a tiny in-process axum mock server; fetch-failure branches use a dead port.

use super::*;
use crate::api::ApiClient;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;
use std::sync::Arc;

const DEAD_BASE: &str = "http://127.0.0.1:9";

/// Spin up an axum server that returns canned `/session` and `/session/status`
/// bodies. Returns `(base_url, abort_handle)`.
async fn mock_server(
    sessions: serde_json::Value,
    status: serde_json::Value,
) -> (String, tokio::task::JoinHandle<()>) {
    use axum::routing::get;
    use axum::{Json, Router};

    let s = Arc::new(sessions);
    let st = Arc::new(status);
    let app = Router::new()
        .route(
            "/session",
            get({
                let s = s.clone();
                move || {
                    let s = s.clone();
                    async move { Json((*s).clone()) }
                }
            }),
        )
        .route(
            "/session/status",
            get({
                let st = st.clone();
                move || {
                    let st = st.clone();
                    async move { Json((*st).clone()) }
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let h = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), h)
}

// ── session_poll_startup_once ────────────────────────────────────────

#[tokio::test]
async fn startup_once_no_projects_is_immediately_ready() {
    // Nothing to hydrate is not a failure to hydrate: with no projects the
    // startup poll succeeds vacuously and the retry loop stops on the first try.
    let h = WebStateHandle::new_test();
    let client = ApiClient::new();
    assert!(h.session_poll_startup_once(&client, DEAD_BASE).await);
    assert!(h.inner.read().await.startup_ready);
}

#[tokio::test]
async fn startup_once_dead_base_returns_false_and_no_sessions() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let client = ApiClient::new();
    assert!(!h.session_poll_startup_once(&client, DEAD_BASE).await);
    assert!(h.inner.read().await.projects[0].sessions.is_empty());
}

#[tokio::test]
async fn startup_once_success_hydrates_active_session() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let client = ApiClient::new();
    let sessions = serde_json::json!([
        { "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } },
        { "id": "sx", "title": "x", "directory": "/other", "time": { "created": 1, "updated": 2 } }
    ]);
    let (base, srv) = mock_server(sessions, serde_json::json!({})).await;
    assert!(h.session_poll_startup_once(&client, &base).await);
    {
        let st = h.inner.read().await;
        // Only the /proj session survives the directory filter.
        assert_eq!(st.projects[0].sessions.len(), 1);
        assert_eq!(st.projects[0].active_session.as_deref(), Some("s1"));
    }
    srv.abort();
}

// ── session_poll_iter_once ───────────────────────────────────────────

#[tokio::test]
async fn iter_once_refreshes_the_session_list() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let mut rx = h.subscribe_events();
    let client = ApiClient::new();
    let sessions = serde_json::json!([
        { "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } }
    ]);
    let status = serde_json::json!({ "s1": { "type": "busy" } });
    let (base, srv) = mock_server(sessions, status).await;
    h.session_poll_iter_once(&client, &base).await;

    // Running status is not this poller's business — it belongs to every
    // runner, and `status.rs` sweeps all of them. This poller owns the list.
    assert_eq!(h.inner.read().await.projects[0].sessions.len(), 1);
    let mut saw_changed = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, WebEvent::StateChanged) {
            saw_changed = true;
        }
    }
    assert!(
        saw_changed,
        "expected StateChanged for the new session list"
    );
    srv.abort();
}

#[tokio::test]
async fn iter_once_no_diff_does_not_emit_changed() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let sess_json = serde_json::json!({ "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } });
    // Pre-seed the project's session list to exactly match what the mock returns.
    {
        let si: crate::app::SessionInfo = serde_json::from_value(sess_json.clone()).unwrap();
        h.inner.write().await.projects[0].sessions = vec![si];
    }
    let mut rx = h.subscribe_events();
    let client = ApiClient::new();
    let (base, srv) = mock_server(serde_json::json!([sess_json]), serde_json::json!({})).await;
    h.session_poll_iter_once(&client, &base).await;

    let mut saw_changed = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, WebEvent::StateChanged) {
            saw_changed = true;
        }
    }
    assert!(
        !saw_changed,
        "identical session list must not emit StateChanged"
    );
    srv.abort();
}

// ── opencode_sse_reconnect_once ──────────────────────────────────────

#[tokio::test]
async fn reconnect_once_no_projects_is_empty() {
    let h = WebStateHandle::new_test();
    let handles = h.opencode_sse_reconnect_once(DEAD_BASE).await;
    assert!(handles.is_empty());
}

#[tokio::test]
async fn reconnect_once_spawns_one_task_per_project() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/a")),
        ("b".into(), PathBuf::from("/b")),
    ]);
    let handles = h.opencode_sse_reconnect_once(DEAD_BASE).await;
    assert_eq!(handles.len(), 2);
    for hh in handles {
        hh.abort();
    }
}

// ── persist_snapshot_once ────────────────────────────────────────────

#[tokio::test]
async fn persist_snapshot_once_writes_to_db() {
    let h = WebStateHandle::new_test();
    h.create_personal_memory(CreatePersonalMemoryRequest {
        label: "L".into(),
        content: "C".into(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
    })
    .await;
    let res = h.persist_snapshot_once().await;
    assert!(matches!(res, Ok(Ok(()))), "expected a clean persist");
    assert_eq!(h.db_for_test().list_memory().len(), 1);
}
