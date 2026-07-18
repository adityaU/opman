use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;

fn ensure_base_url() {
    let _ = crate::app::BASE_URL.get_or_init(|| "http://127.0.0.1:9".to_string());
}

fn mission() -> Mission {
    Mission {
        id: "m1".into(),
        goal: "g".into(),
        session_id: "s1".into(),
        project_index: 0,
        state: MissionState::Pending,
        iteration: 0,
        max_iterations: 5,
        last_verdict: None,
        last_eval_summary: None,
        eval_history: vec![],
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

#[tokio::test]
async fn schedule_persist_without_worker_is_safe() {
    // new_test drops the persist receiver, so the send is a silent no-op.
    let h = WebStateHandle::new_test();
    h.schedule_persist();
}

#[tokio::test]
async fn persist_worker_writes_snapshot_to_db() {
    let h = WebStateHandle::new_test();
    {
        let mut inner = h.inner.write().await;
        inner.missions.insert("m1".into(), mission());
    }
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    h.spawn_persist_worker(rx);
    // Two signals to also exercise the debounce drain (`try_recv` loop).
    tx.send(()).unwrap();
    tx.send(()).unwrap();

    let mut wrote = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if h.db_for_test().list_missions().len() == 1 {
            wrote = true;
            break;
        }
    }
    assert!(wrote, "persist worker did not flush mission to DB");
}

#[tokio::test]
async fn spawn_session_poller_starts_and_polls() {
    ensure_base_url();
    // One project with a path; base_url points at a closed port so fetches
    // fail fast. Drives the eager start-up polling loop.
    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/tmp"))]);
    h.spawn_session_poller();
    // Let a couple of eager-poll attempts run (100ms + 200ms backoff).
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
    // No live opencode server → sessions stay empty, no panic.
    assert!(h.get_project_sessions(0).await.unwrap().2.is_empty());
}

#[tokio::test]
async fn spawn_opencode_sse_listener_starts() {
    ensure_base_url();
    // Exercises the spawn. The listener sleeps 3s before connecting, so the
    // per-project SSE connections are not driven within this test.
    let h = WebStateHandle::new_test();
    h.spawn_opencode_sse_listener();
    tokio::task::yield_now().await;
}
