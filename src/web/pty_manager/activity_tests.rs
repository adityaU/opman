//! Unit tests for foreground-process-group classification.

use super::*;

#[test]
fn shell_owning_its_own_terminal_is_idle() {
    assert_eq!(
        PtyActivity::classify(Some(4242), Some(4242)),
        PtyActivity::Idle
    );
}

#[test]
fn another_process_group_in_the_foreground_is_running() {
    assert_eq!(
        PtyActivity::classify(Some(4243), Some(4242)),
        PtyActivity::Running
    );
}

#[test]
fn unknowable_foreground_group_reports_idle() {
    assert_eq!(PtyActivity::classify(None, Some(4242)), PtyActivity::Idle);
}

#[test]
fn reaped_child_reports_idle() {
    assert_eq!(PtyActivity::classify(Some(4243), None), PtyActivity::Idle);
}

#[test]
fn default_is_idle() {
    assert_eq!(PtyActivity::default(), PtyActivity::Idle);
}

#[test]
fn serializes_lowercase_for_the_sse_payload() {
    let running = serde_json::to_string(&PtyActivity::Running).expect("serializes");
    assert_eq!(running, "\"running\"");
    let idle = serde_json::to_string(&PtyActivity::Idle).expect("serializes");
    assert_eq!(idle, "\"idle\"");
}
