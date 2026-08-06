//! Tests for the shared session-running-status path.
//!
//! The behaviour that matters here is what the old per-runner reconciliation
//! got wrong: a runner that did not answer must not retire another runner's
//! sessions, and a freshly dispatched turn must survive the gap before its
//! runner reports it.

use super::*;
use crate::web::web_state::WebStateHandle;

/// A sweep in which every runner asked answered.
fn sweep(running: &[&str], observed: &[&str]) -> StatusSweep {
    StatusSweep {
        running: running.iter().map(|id| id.to_string()).collect(),
        observed: observed.iter().map(|id| id.to_string()).collect(),
        complete: true,
    }
}

/// A sweep in which at least one runner never answered.
fn partial_sweep(running: &[&str], observed: &[&str]) -> StatusSweep {
    StatusSweep {
        complete: false,
        ..sweep(running, observed)
    }
}

async fn busy_ids(handle: &WebStateHandle) -> Vec<String> {
    let mut ids: Vec<String> = handle
        .inner
        .read()
        .await
        .busy_sessions
        .iter()
        .cloned()
        .collect();
    ids.sort();
    ids
}

async fn label(handle: &WebStateHandle, session_id: &str, runner: &str) {
    handle
        .inner
        .write()
        .await
        .session_runners
        .insert(session_id.to_string(), runner.to_string());
}

#[tokio::test]
async fn busy_then_idle_round_trips() {
    let handle = WebStateHandle::new_test();
    assert!(handle.set_session_running("s1", Running::Busy).await);
    assert_eq!(busy_ids(&handle).await, vec!["s1".to_string()]);
    assert!(handle.set_session_running("s1", Running::Idle).await);
    assert!(busy_ids(&handle).await.is_empty());
}

#[tokio::test]
async fn repeated_observation_is_not_a_transition() {
    let handle = WebStateHandle::new_test();
    assert!(handle.set_session_running("s1", Running::Busy).await);
    assert!(!handle.set_session_running("s1", Running::Busy).await);
    assert!(handle.set_session_running("s1", Running::Idle).await);
    assert!(!handle.set_session_running("s1", Running::Idle).await);
}

#[tokio::test]
async fn busy_clears_the_session_error_state() {
    let handle = WebStateHandle::new_test();
    handle
        .inner
        .write()
        .await
        .error_sessions
        .insert("s1".into(), "boom".into());
    handle.set_session_running("s1", Running::Busy).await;
    assert!(!handle
        .inner
        .read()
        .await
        .error_sessions
        .contains_key("s1"));
}

#[tokio::test]
async fn sweep_marks_reported_sessions_busy() {
    let handle = WebStateHandle::new_test();
    let (busy, idle) = handle
        .apply_status_sweep(&sweep(&["s1", "s2"], &["claude"]))
        .await;
    assert_eq!(busy.len(), 2);
    assert!(idle.is_empty());
    assert_eq!(busy_ids(&handle).await, vec!["s1".to_string(), "s2".into()]);
}

#[tokio::test]
async fn sweep_retires_a_session_its_own_runner_answered_for() {
    let handle = WebStateHandle::new_test();
    label(&handle, "s1", "claude").await;
    handle.set_session_running("s1", Running::Busy).await;

    let (_, idle) = handle.apply_status_sweep(&sweep(&[], &["claude"])).await;
    assert_eq!(idle, vec!["s1".to_string()]);
    assert!(busy_ids(&handle).await.is_empty());
}

#[tokio::test]
async fn sweep_keeps_a_session_whose_runner_did_not_answer() {
    let handle = WebStateHandle::new_test();
    label(&handle, "s1", "claude").await;
    handle.set_session_running("s1", Running::Busy).await;

    // Only opencode answered. Saying nothing about a claude session is not the
    // same as saying it finished — this is the regression the refactor fixes.
    let (_, idle) = handle.apply_status_sweep(&sweep(&[], &["opencode"])).await;
    assert!(idle.is_empty());
    assert_eq!(busy_ids(&handle).await, vec!["s1".to_string()]);
}

#[tokio::test]
async fn an_unlabelled_session_needs_every_runner_to_have_answered() {
    // A subagent, or a session created before opman learned who owns it, cannot
    // be attributed. Only a sweep that heard back from all of them can retire it.
    let handle = WebStateHandle::new_test();
    handle.set_session_running("s1", Running::Busy).await;

    let (_, idle) = handle
        .apply_status_sweep(&partial_sweep(&[], &["opencode"]))
        .await;
    assert!(idle.is_empty());

    let (_, idle) = handle.apply_status_sweep(&sweep(&[], &["opencode"])).await;
    assert_eq!(idle, vec!["s1".to_string()]);
}

#[tokio::test]
async fn a_labelled_session_is_retired_on_its_own_runners_word_alone() {
    // The converse: claude answering is enough for a claude session, even when
    // another runner in the same sweep was unreachable.
    let handle = WebStateHandle::new_test();
    label(&handle, "s1", "claude").await;
    handle.set_session_running("s1", Running::Busy).await;

    let (_, idle) = handle
        .apply_status_sweep(&partial_sweep(&[], &["claude"]))
        .await;
    assert_eq!(idle, vec!["s1".to_string()]);
}

#[tokio::test]
async fn a_sweep_that_reached_nobody_retires_nothing() {
    let handle = WebStateHandle::new_test();
    handle.set_session_running("s1", Running::Busy).await;
    let (_, idle) = handle.apply_status_sweep(&sweep(&[], &[])).await;
    assert!(idle.is_empty());
}

#[tokio::test]
async fn a_dispatched_turn_survives_a_sweep_that_predates_it() {
    let handle = WebStateHandle::new_test();
    label(&handle, "s1", "claude").await;
    handle.mark_turn_dispatched("s1").await;

    // The runner has not registered the turn yet, so it reports nothing.
    let (_, idle) = handle.apply_status_sweep(&sweep(&[], &["claude"])).await;
    assert!(idle.is_empty());
    assert_eq!(busy_ids(&handle).await, vec!["s1".to_string()]);
}

#[tokio::test]
async fn an_expired_dispatch_grace_stops_holding_the_session() {
    let handle = WebStateHandle::new_test();
    label(&handle, "s1", "claude").await;
    handle.mark_turn_dispatched("s1").await;
    handle.inner.write().await.turn_dispatch.insert(
        "s1".into(),
        Instant::now() - DISPATCH_GRACE - Duration::from_secs(1),
    );

    let (_, idle) = handle.apply_status_sweep(&sweep(&[], &["claude"])).await;
    assert_eq!(idle, vec!["s1".to_string()]);
}

#[tokio::test]
async fn settling_a_turn_releases_the_grace_without_forcing_idle() {
    let handle = WebStateHandle::new_test();
    label(&handle, "s1", "claude").await;
    handle.mark_turn_dispatched("s1").await;
    handle.mark_turn_settled("s1").await;

    assert_eq!(busy_ids(&handle).await, vec!["s1".to_string()]);
    let (_, idle) = handle.apply_status_sweep(&sweep(&[], &["claude"])).await;
    assert_eq!(idle, vec!["s1".to_string()]);
}

#[tokio::test]
async fn going_idle_forgets_the_dispatch_grace() {
    let handle = WebStateHandle::new_test();
    handle.mark_turn_dispatched("s1").await;
    handle.set_session_running("s1", Running::Idle).await;
    assert!(!handle.inner.read().await.turn_dispatch.contains_key("s1"));
}

#[tokio::test]
async fn an_untracked_idle_still_counts_unseen() {
    // opman restarted mid-turn: the runner's idle is the first thing it saw.
    let handle = WebStateHandle::new_test();
    assert!(!handle.set_session_running("x9", Running::Idle).await);
    handle.note_untracked_idle("x9").await;
    assert_eq!(handle.inner.read().await.unseen_sessions.get("x9"), Some(&1));
}

#[tokio::test]
async fn a_sweep_with_no_registry_observes_nothing() {
    let handle = WebStateHandle::new_test();
    let sweep = handle.sweep_session_status().await;
    assert!(sweep.running.is_empty());
    assert!(sweep.observed.is_empty());
}

#[tokio::test]
async fn a_sweep_that_finds_a_session_busy_cancels_its_watcher_timer() {
    let handle = WebStateHandle::new_test_with_projects(vec![("p".into(), "/proj".into())]);
    handle
        .create_watcher(WatcherConfigRequest {
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
        let mut state = handle.inner.write().await;
        let pending =
            tokio::spawn(async { tokio::time::sleep(Duration::from_secs(3600)).await })
                .abort_handle();
        state.watcher_pending.insert("s1".into(), pending);
        state.watcher_idle_since.insert("s1".into(), Instant::now());
    }

    handle.apply_status_sweep(&sweep(&["s1"], &["claude"])).await;

    let state = handle.inner.read().await;
    assert!(state.busy_sessions.contains("s1"));
    assert!(!state.watcher_pending.contains_key("s1"));
    assert!(!state.watcher_idle_since.contains_key("s1"));
}

#[tokio::test]
async fn a_sweep_that_retires_a_session_emits_session_idle() {
    let handle = WebStateHandle::new_test();
    label(&handle, "s1", "claude").await;
    handle.set_session_running("s1", Running::Busy).await;
    let mut events = handle.subscribe_events();

    handle.apply_status_sweep(&sweep(&[], &["claude"])).await;

    let mut saw_idle = false;
    while let Ok(event) = events.try_recv() {
        if matches!(event, WebEvent::SessionIdle { ref session_id } if session_id == "s1") {
            saw_idle = true;
        }
    }
    assert!(saw_idle, "expected a SessionIdle event for the cleared session");
}

#[tokio::test]
async fn idle_counts_unseen_for_a_background_root_session() {
    let handle = WebStateHandle::new_test();
    handle.set_session_running("s1", Running::Busy).await;
    handle.set_session_running("s1", Running::Idle).await;
    assert_eq!(
        handle.inner.read().await.unseen_sessions.get("s1"),
        Some(&1)
    );
}
