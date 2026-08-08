use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;
use std::time::Instant;

fn ensure_base_url() {
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:9".to_string());
}

fn req(session_id: &str, project_idx: usize, timeout: u64) -> WatcherConfigRequest {
    WatcherConfigRequest {
        session_id: session_id.into(),
        project_idx,
        idle_timeout_secs: timeout,
        continuation_message: "continue please".into(),
        include_original: true,
        original_message: Some("original".into()),
        hang_message: "hang".into(),
        hang_timeout_secs: 180,
    }
}

fn sess(id: &str, dir: &str) -> crate::app::SessionInfo {
    crate::web::test_support::test_session(id, "", dir, 2)
}

#[tokio::test]
async fn create_watcher_waiting_and_running() {
    let h = WebStateHandle::new_test();
    let mut rx = h.subscribe_events();
    let resp = h.create_watcher(req("s1", 0, 60)).await;
    assert_eq!(resp.session_id, "s1");
    assert_eq!(resp.status, "waiting");
    assert_eq!(resp.continuation_message, "continue please");
    assert!(resp.include_original);
    assert!(matches!(
        rx.try_recv(),
        Ok(WebEvent::WatcherStatusChanged(_))
    ));

    // Busy session → "running"
    h.inner.write().await.busy_sessions.insert("s2".into());
    let resp2 = h.create_watcher(req("s2", 0, 60)).await;
    assert_eq!(resp2.status, "running");
}

#[tokio::test]
async fn delete_watcher_exists_and_missing() {
    let h = WebStateHandle::new_test();
    assert!(!h.delete_watcher("ghost").await);

    h.create_watcher(req("s1", 0, 60)).await;
    // Seed a pending timer + idle marker so their removal branches run.
    {
        let mut inner = h.inner.write().await;
        let ah =
            tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(3600)).await })
                .abort_handle();
        inner.watcher_pending.insert("s1".into(), ah);
        inner.watcher_idle_since.insert("s1".into(), Instant::now());
    }
    let mut rx = h.subscribe_events();
    assert!(h.delete_watcher("s1").await);
    assert!(h.get_watcher("s1").await.is_none());
    // Drain until we see a "deleted" action.
    let mut saw_deleted = false;
    while let Ok(WebEvent::WatcherStatusChanged(ev)) = rx.try_recv() {
        if ev.action == "deleted" {
            saw_deleted = true;
        }
    }
    assert!(saw_deleted);
}

#[tokio::test]
async fn list_watchers_all_statuses() {
    let h = WebStateHandle::new_test_with_projects(vec![("proj".into(), PathBuf::from("/p"))]);
    h.add_and_activate_session(0, sess("running", "/p")).await;
    h.create_watcher(req("running", 0, 60)).await;
    h.create_watcher(req("idle", 0, 60)).await;
    h.create_watcher(req("waiting", 0, 60)).await;
    {
        let mut inner = h.inner.write().await;
        inner.busy_sessions.insert("running".into());
        inner
            .watcher_idle_since
            .insert("idle".into(), Instant::now());
    }
    let entries = h.list_watchers().await;
    assert_eq!(entries.len(), 3);
    let by = |id: &str| entries.iter().find(|e| e.session_id == id).unwrap().clone();
    assert_eq!(by("running").status, "running");
    let idle = by("idle");
    assert_eq!(idle.status, "idle_countdown");
    assert!(idle.idle_since_secs.is_some());
    assert_eq!(by("waiting").status, "waiting");
    // Title + project resolved for the session that exists in the project.
    assert_eq!(by("running").session_title, "title-running");
    assert_eq!(by("running").project_name, "proj");
    // For a session id absent from the project, title falls back to the id.
    assert_eq!(by("waiting").session_title, "waiting");
}

#[tokio::test]
async fn get_watcher_status_variants_and_missing() {
    let h = WebStateHandle::new_test();
    assert!(h.get_watcher("none").await.is_none());

    h.create_watcher(req("s1", 0, 45)).await;
    // waiting
    let w = h.get_watcher("s1").await.unwrap();
    assert_eq!(w.status, "waiting");
    assert_eq!(w.idle_timeout_secs, 45);

    // idle_countdown
    h.inner
        .write()
        .await
        .watcher_idle_since
        .insert("s1".into(), Instant::now());
    assert_eq!(h.get_watcher("s1").await.unwrap().status, "idle_countdown");

    // running (busy wins over idle marker)
    h.inner.write().await.busy_sessions.insert("s1".into());
    assert_eq!(h.get_watcher("s1").await.unwrap().status, "running");
}

#[tokio::test]
async fn get_watcher_sessions_flags() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/a")),
        ("b".into(), PathBuf::from("/b")),
    ]);
    h.add_and_activate_session(0, sess("s1", "/a")).await; // active project 0, active session
    h.add_and_activate_session(1, sess("s2", "/b")).await;
    {
        let mut inner = h.inner.write().await;
        inner.busy_sessions.insert("s2".into());
    }
    h.create_watcher(req("s1", 0, 60)).await;

    let entries = h.get_watcher_sessions().await;
    let s1 = entries.iter().find(|e| e.session_id == "s1").unwrap();
    assert!(s1.is_current); // active session in active project
    assert!(s1.has_watcher);
    assert!(!s1.is_active);
    let s2 = entries.iter().find(|e| e.session_id == "s2").unwrap();
    assert!(!s2.is_current); // project 1 is not the active project
    assert!(s2.is_active);
    assert!(!s2.has_watcher);
}

#[tokio::test]
async fn try_trigger_watcher_no_watcher_is_noop() {
    let h = WebStateHandle::new_test();
    h.try_trigger_watcher("nobody").await;
    assert!(h.inner.read().await.watcher_pending.is_empty());
}

#[tokio::test]
async fn try_trigger_watcher_suppressed_by_active_children() {
    let h = WebStateHandle::new_test();
    h.create_watcher(req("parent", 0, 3600)).await;
    {
        let mut inner = h.inner.write().await;
        let mut kids = std::collections::HashSet::new();
        kids.insert("child".to_string());
        inner.session_children.insert("parent".into(), kids);
        inner.busy_sessions.insert("child".into());
        inner
            .watcher_idle_since
            .insert("parent".into(), Instant::now());
    }
    h.try_trigger_watcher("parent").await;
    let inner = h.inner.read().await;
    // Suppressed: no timer scheduled, idle marker cleared.
    assert!(inner.watcher_pending.is_empty());
    assert!(!inner.watcher_idle_since.contains_key("parent"));
}

#[tokio::test]
async fn try_trigger_watcher_schedules_timer() {
    ensure_base_url();
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    // Long timeout so the spawned continuation never fires during the test.
    h.create_watcher(req("s1", 0, 3600)).await;
    // Pre-existing pending timer to exercise the abort-of-previous branch.
    {
        let mut inner = h.inner.write().await;
        let ah =
            tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(3600)).await })
                .abort_handle();
        inner.watcher_pending.insert("s1".into(), ah);
    }
    let mut rx = h.subscribe_events();
    h.try_trigger_watcher("s1").await;
    {
        let inner = h.inner.read().await;
        assert!(inner.watcher_pending.contains_key("s1"));
        assert!(inner.watcher_idle_since.contains_key("s1"));
    }
    let mut saw_countdown = false;
    while let Ok(WebEvent::WatcherStatusChanged(ev)) = rx.try_recv() {
        if ev.action == "countdown" {
            saw_countdown = true;
        }
    }
    assert!(saw_countdown);
    // Clean up the spawned timer.
    h.cancel_watcher_timer("s1").await;
}

#[tokio::test]
async fn cancel_watcher_timer_with_and_without_watcher() {
    let h = WebStateHandle::new_test();
    // No watcher registered → no-op.
    h.cancel_watcher_timer("s1").await;

    h.create_watcher(req("s1", 0, 3600)).await;
    {
        let mut inner = h.inner.write().await;
        let ah =
            tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(3600)).await })
                .abort_handle();
        inner.watcher_pending.insert("s1".into(), ah);
        inner.watcher_idle_since.insert("s1".into(), Instant::now());
    }
    let mut rx = h.subscribe_events();
    h.cancel_watcher_timer("s1").await;
    let inner = h.inner.read().await;
    assert!(!inner.watcher_pending.contains_key("s1"));
    assert!(!inner.watcher_idle_since.contains_key("s1"));
    drop(inner);
    let mut saw_cancelled = false;
    while let Ok(WebEvent::WatcherStatusChanged(ev)) = rx.try_recv() {
        if ev.action == "cancelled" {
            saw_cancelled = true;
        }
    }
    assert!(saw_cancelled);
}
