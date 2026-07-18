use super::*;
use serde_json::json;
use std::path::Path;

#[test]
fn socket_path_is_deterministic_and_formatted() {
    let p = Path::new("/home/user/project");
    let a = socket_path_for_project(p);
    let b = socket_path_for_project(p);
    assert_eq!(a, b);
    let s = a.to_string_lossy();
    assert!(s.starts_with("/tmp/opman-"));
    assert!(s.ends_with(".sock"));
}

#[test]
fn socket_path_differs_for_different_projects() {
    let a = socket_path_for_project(Path::new("/a"));
    let b = socket_path_for_project(Path::new("/b"));
    assert_ne!(a, b);
}

#[test]
fn socket_response_ok_text() {
    let r = SocketResponse::ok_text("hello".into());
    assert!(r.ok);
    assert_eq!(r.output.as_deref(), Some("hello"));
    assert!(r.tabs.is_none());
    assert!(r.error.is_none());
    assert!(r.tab_index.is_none());
    assert!(r.command_state.is_none());
}

#[test]
fn socket_response_ok_tabs() {
    let r = SocketResponse::ok_tabs(vec![TabInfo {
        index: 0,
        active: true,
        name: "n".into(),
    }]);
    assert!(r.ok);
    assert_eq!(r.tabs.as_ref().unwrap().len(), 1);
    assert!(r.output.is_none());
}

#[test]
fn socket_response_ok_tab_created() {
    let r = SocketResponse::ok_tab_created(5);
    assert!(r.ok);
    assert_eq!(r.tab_index, Some(5));
}

#[test]
fn socket_response_ok_empty_and_status() {
    let e = SocketResponse::ok_empty();
    assert!(e.ok);
    assert!(e.output.is_none() && e.tabs.is_none() && e.tab_index.is_none());
    let s = SocketResponse::ok_status("running".into());
    assert!(s.ok);
    assert_eq!(s.command_state.as_deref(), Some("running"));
}

#[test]
fn socket_response_err() {
    let r = SocketResponse::err("bad".into());
    assert!(!r.ok);
    assert_eq!(r.error.as_deref(), Some("bad"));
}

#[test]
fn socket_response_serialize_skips_none() {
    let r = SocketResponse::ok_text("x".into());
    let v = serde_json::to_value(&r).unwrap();
    // None fields are skipped.
    assert!(v.get("tabs").is_none());
    assert!(v.get("error").is_none());
    assert!(v.get("tab_index").is_none());
    assert!(v.get("command_state").is_none());
    assert_eq!(v["ok"], true);
    assert_eq!(v["output"], "x");
}

#[test]
fn socket_response_roundtrip() {
    let r = SocketResponse::ok_tab_created(3);
    let s = serde_json::to_string(&r).unwrap();
    let back: SocketResponse = serde_json::from_str(&s).unwrap();
    assert_eq!(back.tab_index, Some(3));
    assert!(back.ok);
}

#[test]
fn tab_info_roundtrip() {
    let t = TabInfo {
        index: 2,
        active: false,
        name: "build".into(),
    };
    let s = serde_json::to_string(&t).unwrap();
    let back: TabInfo = serde_json::from_str(&s).unwrap();
    assert_eq!(back.index, 2);
    assert!(!back.active);
    assert_eq!(back.name, "build");
}

#[test]
fn edit_op_roundtrip() {
    let e = EditOp {
        file_path: "src/x.rs".into(),
        start_line: 1,
        end_line: 4,
        new_text: "hi".into(),
    };
    let s = serde_json::to_string(&e).unwrap();
    let back: EditOp = serde_json::from_str(&s).unwrap();
    assert_eq!(back.file_path, "src/x.rs");
    assert_eq!(back.start_line, 1);
    assert_eq!(back.end_line, 4);
    assert_eq!(back.new_text, "hi");
}

#[test]
fn socket_request_default() {
    let r = SocketRequest::default();
    assert_eq!(r.op, "");
    assert!(r.session_id.is_none());
    assert!(r.tab.is_none());
    assert!(r.edits.is_none());
}

#[test]
fn socket_request_deserialize_minimal() {
    let r: SocketRequest = serde_json::from_str(r#"{"op":"read"}"#).unwrap();
    assert_eq!(r.op, "read");
    assert!(r.tab.is_none());
    assert!(r.command.is_none());
}

#[test]
fn socket_request_deserialize_full_and_skip_serialize() {
    let json_in = json!({
        "op": "nvim_edit_and_save",
        "session_id": "s1",
        "tab": 2,
        "command": "w",
        "name": "tab",
        "wait": true,
        "last_n": 10,
        "file_path": "a.rs",
        "line": 3,
        "end_line": 9,
        "col": 4,
        "query": "q",
        "buf_only": true,
        "workspace": false,
        "all": true,
        "glob": "*.rs",
        "new_text": "z",
        "count": -1,
        "new_name": "y",
        "edits": [{"file_path":"a.rs","start_line":1,"end_line":2,"new_text":"t"}]
    });
    let r: SocketRequest = serde_json::from_value(json_in).unwrap();
    assert_eq!(r.op, "nvim_edit_and_save");
    assert_eq!(r.session_id.as_deref(), Some("s1"));
    assert_eq!(r.tab, Some(2));
    assert_eq!(r.count, Some(-1));
    assert_eq!(r.edits.as_ref().unwrap().len(), 1);

    // Serialization: skip_serializing_if for None-y fields on a mostly-empty req.
    let empty = SocketRequest {
        op: "list".into(),
        ..Default::default()
    };
    let v = serde_json::to_value(&empty).unwrap();
    assert_eq!(v["op"], "list");
    assert!(v.get("session_id").is_none());
    assert!(v.get("name").is_none());
    assert!(v.get("edits").is_none());
    // `tab`, `command` have no skip_serializing_if, so they serialize as null.
    assert!(v.get("tab").is_some());
    assert_eq!(v["tab"], serde_json::Value::Null);
}

#[test]
fn new_registry_starts_empty() {
    let reg = new_nvim_socket_registry();
    // Reading synchronously via try_read on an uncontended lock.
    let g = reg.try_read().unwrap();
    assert!(g.is_empty());
}

#[test]
fn cleanup_socket_removes_existing_file() {
    // Use a unique project path so the socket file is unique to this test.
    let unique = format!("/tmp/opman-cleanup-test-{}", std::process::id());
    let proj = Path::new(&unique);
    let sock = socket_path_for_project(proj);
    std::fs::write(&sock, b"x").unwrap();
    assert!(sock.exists());
    cleanup_socket(proj);
    assert!(!sock.exists());
    // Removing again (missing file) must not panic.
    cleanup_socket(proj);
}
