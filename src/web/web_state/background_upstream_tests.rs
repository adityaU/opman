//! Extra mock-upstream branch coverage for `background.rs` poll iterations that
//! the existing single-iteration tests don't reach: the startup poll's
//! "active session already set" and "no matching sessions" branches, and the
//! recurring poll's busy-transition path that cancels a pending watcher timer.
use super::*;
use crate::api::ApiClient;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

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

// ── session_poll_startup_once: active_session already set ────────────

#[tokio::test]
async fn startup_once_keeps_existing_active_session() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    // A valid active session must survive hydration.
    h.inner.write().await.projects[0].active_session = Some("s1".into());
    let client = ApiClient::new();
    let sessions = serde_json::json!([
        { "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } }
    ]);
    let (base, srv) = mock_server(sessions, serde_json::json!({})).await;
    assert!(h.session_poll_startup_once(&client, &base).await);
    let st = h.inner.read().await;
    // Session list refreshed, and the valid user selection is untouched.
    assert_eq!(st.projects[0].sessions.len(), 1);
    assert_eq!(st.projects[0].active_session.as_deref(), Some("s1"));
    drop(st);
    srv.abort();
}

#[tokio::test]
async fn startup_once_repairs_stale_active_session() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    h.inner.write().await.projects[0].active_session = Some("ghost-session".into());
    let client = ApiClient::new();
    let sessions = serde_json::json!([
        { "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } }
    ]);
    let (base, srv) = mock_server(sessions, serde_json::json!({})).await;

    assert!(h.session_poll_startup_once(&client, &base).await);
    assert_eq!(
        h.inner.read().await.projects[0].active_session.as_deref(),
        Some("s1")
    );
    srv.abort();
}

// ── session_poll_startup_once: fetch ok but nothing matches the dir ──

#[tokio::test]
async fn startup_once_no_matching_sessions_leaves_active_none() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let client = ApiClient::new();
    // Only a session for a different directory → filtered list is empty, so the
    // `filtered.first()` branch yields None and active_session stays None.
    let sessions = serde_json::json!([
        { "id": "sx", "title": "x", "directory": "/other", "time": { "created": 1, "updated": 2 } }
    ]);
    let (base, srv) = mock_server(sessions, serde_json::json!({})).await;
    assert!(h.session_poll_startup_once(&client, &base).await);
    let st = h.inner.read().await;
    assert!(st.projects[0].sessions.is_empty());
    assert!(st.projects[0].active_session.is_none());
    drop(st);
    srv.abort();
}

// ── session_poll_iter_once: busy transition cancels a pending watcher ──

#[tokio::test]
async fn iter_once_busy_transition_cancels_watcher_timer() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    // A watcher on s1 with a live pending timer + idle marker; the poller should
    // cancel it when s1 flips to busy.
    h.create_watcher(WatcherConfigRequest {
        session_id: "s1".into(),
        project_idx: 0,
        idle_timeout_secs: 3600,
        continuation_message: "c".into(),
        include_original: false,
        original_message: None,
        hang_message: "h".into(),
        hang_timeout_secs: 180,
    })
    .await;
    {
        let mut inner = h.inner.write().await;
        let ah =
            tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(3600)).await })
                .abort_handle();
        inner.watcher_pending.insert("s1".into(), ah);
        inner.watcher_idle_since.insert("s1".into(), Instant::now());
    }

    let client = ApiClient::new();
    let sessions = serde_json::json!([
        { "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } }
    ]);
    // s1 busy (goes into aggregated_busy); s2 idle is skipped by the `!= "idle"` guard.
    let status = serde_json::json!({ "s1": { "type": "busy" }, "s2": { "type": "idle" } });
    let (base, srv) = mock_server(sessions, status).await;
    h.session_poll_iter_once(&client, &base).await;

    let inner = h.inner.read().await;
    assert!(inner.busy_sessions.contains("s1"));
    // The busy transition fired cancel_watcher_timer.
    assert!(!inner.watcher_pending.contains_key("s1"));
    assert!(!inner.watcher_idle_since.contains_key("s1"));
    drop(inner);
    srv.abort();
}
