//! Generated tests for the web PTY manager loop.
//!
//! We start a real manager thread (which spawns NO child processes on its own)
//! and drive the non-spawn command arms against an empty PTY table. The spawn
//! arms (SpawnShell/Neovim/Gitui/Opencode/ClaudeAttach) launch real external
//! programs and are intentionally not exercised here.

use super::*;

#[tokio::test]
async fn list_is_empty_on_fresh_manager() {
    let h = start_web_pty_manager();
    assert!(h.list().await.is_empty());
    // Dropping `h` closes the command channel and the manager thread exits.
}

#[tokio::test]
async fn control_commands_on_missing_pty_return_false_or_none() {
    let h = start_web_pty_manager();

    // No PTY with this id exists -> every control op reports "not found".
    assert!(
        !h.write("nope", vec![1, 2, 3]).await,
        "write to missing pty"
    );
    assert!(!h.resize("nope", 40, 100).await, "resize missing pty");
    assert!(!h.kill("nope").await, "kill missing pty");
    assert!(
        h.get_output("nope").await.is_none(),
        "get_output missing pty"
    );
    assert!(h.list().await.is_empty());
}
