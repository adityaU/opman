//! Success-path tests for the MCP terminal handlers, driven against a REAL
//! `WebPtyHandle` (a live PTY-manager thread spawning an actual shell).
//!
//! These complement `terminal_tests.rs` (which covers the no-op / error
//! branches) by exercising the spawn → write → read → list → close lifecycle.
//! Asserts are lenient about exact bytes/timing (a real shell's prompt and echo
//! are environment-dependent) but strict about the branch that runs. Every test
//! kills the PTYs it spawns so no shell leaks. If the environment cannot spawn a
//! PTY at all (`spawn_shell` errors), the lifecycle tests bail out early rather
//! than fail — the handler code they'd cover is unreachable there.

use super::*;
use crate::web::test_support::test_server_state;
use crate::web::types::{ServerState, WebEvent};

/// A test state whose `pty_mgr` is a live manager thread.
fn live_state() -> ServerState {
    let mut s = test_server_state();
    s.pty_mgr = crate::web::pty_manager::start_web_pty_manager();
    s
}

/// Spawn a shell via `handle_terminal_new`; return its id, or `None` if the
/// environment can't spawn a PTY (so callers can skip gracefully).
async fn spawn_terminal(state: &ServerState) -> Option<String> {
    match handle_terminal_new(state, &serde_json::json!({"rows": 24, "cols": 80})).await {
        Ok(out) => {
            let v: serde_json::Value = serde_json::from_str(&out).unwrap();
            Some(v["id"].as_str().unwrap().to_string())
        }
        Err(_) => None,
    }
}

#[tokio::test]
async fn new_spawns_and_returns_id_rows_cols() {
    let state = live_state();
    let Ok(out) = handle_terminal_new(&state, &serde_json::json!({"rows": 30, "cols": 100})).await
    else {
        return; // no PTY support in this environment
    };
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let id = v["id"].as_str().unwrap();
    assert!(!id.is_empty());
    assert_eq!(v["rows"], 30);
    assert_eq!(v["cols"], 100);
    // Clean up.
    let _ = handle_terminal_close(&state, &serde_json::json!({"id": id})).await;
}

#[tokio::test]
async fn run_no_wait_sends_command_and_emits_focus() {
    let state = live_state();
    let Some(id) = spawn_terminal(&state).await else {
        return;
    };

    let mut rx = state.event_tx.subscribe();
    let msg = handle_terminal_run(
        &state,
        &serde_json::json!({"id": id, "command": "echo hello"}),
    )
    .await
    .unwrap();
    assert_eq!(msg, "Command sent");

    // The focus event must have been broadcast for this id.
    let mut saw_focus = false;
    while let Ok(ev) = rx.try_recv() {
        if let WebEvent::McpTerminalFocus { id: fid } = ev {
            if fid == id {
                saw_focus = true;
            }
        }
    }
    assert!(saw_focus, "McpTerminalFocus event was not emitted");

    let _ = handle_terminal_close(&state, &serde_json::json!({"id": id})).await;
}

#[tokio::test]
async fn run_wait_collects_output_or_fallback() {
    let state = live_state();
    let Some(id) = spawn_terminal(&state).await else {
        return;
    };

    // wait=true with a small timeout drives the settle/poll loop.
    let args =
        serde_json::json!({"id": id, "command": "echo settle-marker", "wait": true, "timeout": 2});
    let fut = handle_terminal_run(&state, &args);
    let out = tokio::time::timeout(std::time::Duration::from_secs(6), fut)
        .await
        .expect("run(wait) did not settle in time")
        .unwrap();
    // Either the echoed marker landed, or the no-output fallback fired.
    assert!(
        out.contains("settle-marker") || out.contains("[command sent, no output captured]"),
        "unexpected wait output: {out:?}"
    );

    let _ = handle_terminal_close(&state, &serde_json::json!({"id": id})).await;
}

#[tokio::test]
async fn read_returns_output_then_drains_empty() {
    let state = live_state();
    let Some(id) = spawn_terminal(&state).await else {
        return;
    };

    // Produce some output and let it flush.
    let _ = handle_terminal_run(
        &state,
        &serde_json::json!({"id": id, "command": "echo read-marker"}),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let first = handle_terminal_read(&state, &serde_json::json!({"id": id}))
        .await
        .unwrap();
    assert_ne!(
        first, "[no new output]",
        "expected real output on first read"
    );

    // A second immediate read has nothing new to drain.
    let second = handle_terminal_read(&state, &serde_json::json!({"id": id}))
        .await
        .unwrap();
    assert_eq!(second, "[no new output]");

    let _ = handle_terminal_close(&state, &serde_json::json!({"id": id})).await;
}

#[tokio::test]
async fn read_last_n_slices_to_single_line() {
    let state = live_state();
    let Some(id) = spawn_terminal(&state).await else {
        return;
    };

    // Drain the shell's startup/prompt output first.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let _ = handle_terminal_read(&state, &serde_json::json!({"id": id})).await;

    let _ = handle_terminal_run(
        &state,
        &serde_json::json!({"id": id, "command": "echo l1; echo l2; echo l3"}),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let out = handle_terminal_read(&state, &serde_json::json!({"id": id, "last_n": 1}))
        .await
        .unwrap();
    if out != "[no new output]" {
        // last_n=1 joins exactly one line → no embedded newline.
        assert!(
            !out.contains('\n'),
            "last_n=1 should yield a single line, got {out:?}"
        );
    }

    let _ = handle_terminal_close(&state, &serde_json::json!({"id": id})).await;
}

#[tokio::test]
async fn list_includes_live_terminal() {
    let state = live_state();
    let Some(id) = spawn_terminal(&state).await else {
        return;
    };

    let out = handle_terminal_list(&state).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["count"].as_u64().unwrap() >= 1);
    let ids: Vec<&str> = v["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x.as_str())
        .collect();
    assert!(ids.contains(&id.as_str()));

    let _ = handle_terminal_close(&state, &serde_json::json!({"id": id})).await;
}

#[tokio::test]
async fn close_live_then_missing() {
    let state = live_state();
    let Some(id) = spawn_terminal(&state).await else {
        return;
    };

    let ok = handle_terminal_close(&state, &serde_json::json!({"id": id}))
        .await
        .unwrap();
    assert!(ok.contains("closed"));

    // Closing again → not found.
    let err = handle_terminal_close(&state, &serde_json::json!({"id": id}))
        .await
        .unwrap_err();
    assert!(err.contains("not found"));
}
