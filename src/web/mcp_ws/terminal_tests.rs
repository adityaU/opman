//! Generated tests for the MCP terminal tool handlers.
//!
//! The test `ServerState` has a no-op PTY handle: spawn/write/get_output/list/kill
//! all fail fast. So we cover argument parsing and the "not found / failed"
//! branches. The success paths that require a live PTY (read returning real
//! bytes, run with wait=true collecting output, terminal_new returning an id,
//! terminal_close on a live PTY) can't be reached without spawning a process
//! and are noted in the module report.

use super::*;
use crate::web::test_support::test_server_state;

#[tokio::test]
async fn read_missing_id_errors() {
    let s = test_server_state();
    let err = handle_terminal_read(&s, &serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(err.contains("Missing required 'id'"));
}

#[tokio::test]
async fn read_unknown_pty_errors() {
    let s = test_server_state();
    // get_output on the no-op handle returns None.
    let err = handle_terminal_read(&s, &serde_json::json!({"id": "abc", "last_n": 5}))
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn run_missing_id_and_command() {
    let s = test_server_state();
    assert!(handle_terminal_run(&s, &serde_json::json!({}))
        .await
        .is_err());
    assert!(handle_terminal_run(&s, &serde_json::json!({"id": "x"}))
        .await
        .unwrap_err()
        .contains("Missing required 'command'"));
}

#[tokio::test]
async fn run_write_failure_reports_error() {
    let s = test_server_state();
    // write on the no-op handle returns false -> "Failed to write".
    let err = handle_terminal_run(&s, &serde_json::json!({"id": "x", "command": "ls"}))
        .await
        .unwrap_err();
    assert!(err.contains("Failed to write"));
}

#[tokio::test]
async fn run_ctrl_c_command_takes_no_newline_branch() {
    // Command starting with \x03 skips the appended newline; still fails at write
    // (no-op handle) but exercises that branch.
    let s = test_server_state();
    let err = handle_terminal_run(
        &s,
        &serde_json::json!({"id": "x", "command": "\u{0003}", "wait": true, "timeout": 1}),
    )
    .await
    .unwrap_err();
    assert!(err.contains("Failed to write"));
}

#[tokio::test]
async fn list_returns_empty_json() {
    let s = test_server_state();
    let out = handle_terminal_list(&s).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["count"], 0);
    assert!(v["terminals"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn new_spawn_failure_reports_error() {
    let s = test_server_state();
    // spawn_shell on the no-op handle errors ("PTY manager not running").
    let err = handle_terminal_new(&s, &serde_json::json!({"rows": 30, "cols": 100}))
        .await
        .unwrap_err();
    assert!(err.contains("Failed to spawn PTY"));
}

#[tokio::test]
async fn new_uses_defaults_when_size_absent() {
    let s = test_server_state();
    // Still fails at spawn, but exercises the default rows=24/cols=80 branch.
    assert!(handle_terminal_new(&s, &serde_json::json!({}))
        .await
        .is_err());
}

#[tokio::test]
async fn close_missing_id_errors() {
    let s = test_server_state();
    assert!(handle_terminal_close(&s, &serde_json::json!({}))
        .await
        .is_err());
}

#[tokio::test]
async fn close_unknown_pty_errors() {
    let s = test_server_state();
    // kill on the no-op handle returns false -> "not found".
    let err = handle_terminal_close(&s, &serde_json::json!({"id": "abc"}))
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}
