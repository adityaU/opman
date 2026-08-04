//! Mock-upstream coverage for `watchers.rs`: the continuation message that the
//! watcher timer sends when a session stays idle. `try_trigger_watcher` captures
//! `base_url()` (task-local override) before spawning the timer, so with a zero
//! idle-timeout the spawned continuation fires immediately against a live mock,
//! exercising the `send_system_message_async` success path and the "triggered"
//! status event.
use super::*;
use crate::web::test_support::{scope_base_url, start_mock_upstream};
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;

fn req(session_id: &str, timeout: u64, include_original: bool) -> WatcherConfigRequest {
    WatcherConfigRequest {
        session_id: session_id.into(),
        project_idx: 0,
        idle_timeout_secs: timeout,
        continuation_message: "carry on".into(),
        include_original,
        original_message: Some("the original ask".into()),
        hang_message: "hang".into(),
        hang_timeout_secs: 180,
    }
}

fn prompt_mock() -> axum::Router {
    use axum::routing::post;
    axum::Router::new().route(
        "/session/{id}/prompt_async",
        post(|| async { axum::Json(serde_json::json!({ "ok": true })) }),
    )
}

async fn await_triggered(rx: &mut tokio::sync::broadcast::Receiver<WebEvent>) -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match rx.recv().await {
                Ok(WebEvent::WatcherStatusChanged(ev)) if ev.action == "triggered" => break true,
                Ok(_) => continue,
                Err(_) => break false,
            }
        }
    })
    .await
    .unwrap_or(false)
}

#[tokio::test]
async fn continuation_fires_to_upstream_with_original() {
    let base = start_mock_upstream(prompt_mock()).await;
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    // Zero timeout ⇒ the spawned continuation runs at once. include_original true
    // ⇒ the `[Original message]` prefix branch executes.
    h.create_watcher(req("s1", 0, true)).await;
    let mut rx = h.subscribe_events();
    let h2 = h.clone();
    scope_base_url(base, async move { h2.try_trigger_watcher("s1").await }).await;
    assert!(
        await_triggered(&mut rx).await,
        "expected a 'triggered' watcher event"
    );
}

#[tokio::test]
async fn continuation_fires_without_original() {
    let base = start_mock_upstream(prompt_mock()).await;
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    // include_original false ⇒ `original` is None and the prefix branch is skipped.
    h.create_watcher(req("s2", 0, false)).await;
    let mut rx = h.subscribe_events();
    let h2 = h.clone();
    scope_base_url(base, async move { h2.try_trigger_watcher("s2").await }).await;
    assert!(
        await_triggered(&mut rx).await,
        "expected a 'triggered' watcher event"
    );
}
