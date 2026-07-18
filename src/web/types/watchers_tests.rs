use super::*;
use serde_json::json;

#[test]
fn default_helpers() {
    assert_eq!(default_hang_timeout(), 180);
    assert_eq!(
        default_hang_message(),
        "The previous attempt appears to have stalled. Please retry the task."
    );
}

#[test]
fn watcher_config_request_full() {
    let r: WatcherConfigRequest = serde_json::from_value(json!({
        "session_id": "s",
        "project_idx": 2,
        "idle_timeout_secs": 30,
        "continuation_message": "continue",
        "include_original": true,
        "original_message": "orig",
        "hang_message": "stalled",
        "hang_timeout_secs": 90
    }))
    .unwrap();
    assert_eq!(r.session_id, "s");
    assert_eq!(r.project_idx, 2);
    assert_eq!(r.idle_timeout_secs, 30);
    assert_eq!(r.continuation_message, "continue");
    assert!(r.include_original);
    assert_eq!(r.original_message.as_deref(), Some("orig"));
    assert_eq!(r.hang_message, "stalled");
    assert_eq!(r.hang_timeout_secs, 90);
    let _ = r.clone();
}

#[test]
fn watcher_config_request_defaults() {
    let r: WatcherConfigRequest = serde_json::from_value(json!({
        "session_id": "s",
        "project_idx": 0,
        "idle_timeout_secs": 10,
        "continuation_message": "go",
        "original_message": null
    }))
    .unwrap();
    // include_original defaults false.
    assert!(!r.include_original);
    // hang_message/hang_timeout use default fns.
    assert_eq!(r.hang_timeout_secs, 180);
    assert_eq!(
        r.hang_message,
        "The previous attempt appears to have stalled. Please retry the task."
    );
    assert!(r.original_message.is_none());
}

#[test]
fn watcher_config_response_serialize() {
    let resp = WatcherConfigResponse {
        session_id: "s".into(),
        project_idx: 1,
        idle_timeout_secs: 20,
        continuation_message: "c".into(),
        include_original: true,
        original_message: Some("o".into()),
        hang_message: "h".into(),
        hang_timeout_secs: 60,
        status: "running".into(),
        idle_since_secs: Some(5),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["session_id"], "s");
    assert_eq!(v["status"], "running");
    assert_eq!(v["idle_since_secs"], 5);
    assert_eq!(v["original_message"], "o");
    assert!(format!("{:?}", resp.clone()).contains("WatcherConfigResponse"));
}

#[test]
fn watcher_list_entry_serialize() {
    let e = WatcherListEntry {
        session_id: "s".into(),
        session_title: "T".into(),
        project_name: "p".into(),
        idle_timeout_secs: 30,
        status: "idle_countdown".into(),
        idle_since_secs: None,
    };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["session_title"], "T");
    assert_eq!(v["status"], "idle_countdown");
    assert!(v["idle_since_secs"].is_null());
    let _ = e.clone();
}

#[test]
fn watcher_session_entry_serialize() {
    let e = WatcherSessionEntry {
        session_id: "s".into(),
        title: "T".into(),
        project_name: "p".into(),
        project_idx: 3,
        is_current: true,
        is_active: false,
        has_watcher: true,
    };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["project_idx"], 3);
    assert_eq!(v["is_current"], true);
    assert_eq!(v["is_active"], false);
    assert_eq!(v["has_watcher"], true);
    let _ = e.clone();
}

#[test]
fn watcher_status_event_serialize() {
    let ev = WatcherStatusEvent {
        session_id: "s".into(),
        action: "triggered".into(),
        idle_since_secs: Some(12),
    };
    let v = serde_json::to_value(&ev).unwrap();
    assert_eq!(v["action"], "triggered");
    assert_eq!(v["idle_since_secs"], 12);
    assert!(format!("{:?}", ev.clone()).contains("WatcherStatusEvent"));
}

#[test]
fn watcher_message_entry_serialize() {
    let m = WatcherMessageEntry {
        role: "user".into(),
        text: "hello".into(),
    };
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["role"], "user");
    assert_eq!(v["text"], "hello");
    let _ = m.clone();
}
