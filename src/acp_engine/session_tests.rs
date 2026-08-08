//! What a session reports about itself.
//!
//! The engine has always held the user's model, agent, effort and permission mode, and
//! always written them to disk. It just never reported them — so the composer had no way
//! to know what a session was configured as, and fell back to a value shared by every
//! session on the same runner. These tests pin the reporting.

use super::*;

fn configured() -> Session {
    Session {
        id: "ses_1".to_string(),
        title: "Test".to_string(),
        directory: "/tmp/proj".to_string(),
        model: Some("gpt-5-codex".to_string()),
        agent: Some("plan".to_string()),
        effort: Some("high".to_string()),
        permission_mode: Some("acceptEdits".to_string()),
        ..Session::default()
    }
}

#[test]
fn a_session_reports_what_it_is_configured_to_run_as() {
    let reported = session_info(&configured());
    assert_eq!(reported["engine"]["model"], "gpt-5-codex");
    assert_eq!(reported["engine"]["agent"], "plan");
    assert_eq!(reported["engine"]["effort"], "high");
    assert_eq!(reported["engine"]["permissionMode"], "acceptEdits");
}

#[test]
fn a_session_that_was_never_configured_claims_nothing() {
    // Absent, not empty: a runner answers "never chosen" with its own current default,
    // and reporting an empty string instead would pin the composer to a value that
    // matches nothing in the catalogue.
    let reported = session_info(&Session::default());
    assert!(reported["engine"]["model"].is_null());
    assert!(reported["engine"]["agent"].is_null());
    assert!(reported["engine"]["effort"].is_null());
    assert!(reported["engine"]["permissionMode"].is_null());
}

#[test]
fn the_reported_shape_is_what_the_session_list_reads_back() {
    // The engines emit JSON and the web layer deserializes it into `app::SessionInfo`.
    // A rename on either side breaks the whole chain silently, so round-trip it.
    let decoded: crate::app::SessionInfo =
        serde_json::from_value(session_info(&configured())).expect("decode session");
    assert_eq!(decoded.engine.model.as_deref(), Some("gpt-5-codex"));
    assert_eq!(
        decoded.engine.permission_mode.as_deref(),
        Some("acceptEdits")
    );
}

#[test]
fn the_choices_survive_a_round_trip_through_disk() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = Some(dir.path().join("sessions.json"));
    let sessions = HashMap::from([("ses_1".to_string(), configured())]);

    save_sessions(&path, &sessions);
    let loaded = load_sessions(&path);

    let session = loaded.get("ses_1").expect("session survived");
    assert_eq!(session.model.as_deref(), Some("gpt-5-codex"));
    assert_eq!(session.effort.as_deref(), Some("high"));
    assert_eq!(session.permission_mode.as_deref(), Some("acceptEdits"));
}
