//! Wave-2 tests driving a real PTY manager's spawn + control arms with FAKE
//! programs on PATH/$SHELL, covering the `Ok(pty)` insert branches and the
//! write/resize/get_output/kill arms against a present PTY.

use super::super::activity::PtyActivity;
use super::super::handle::WebPtyHandle;
use super::pty_test_support::{env_lock, write_fake_bin, EnvRestore};
use super::*;

#[tokio::test]
async fn shell_pty_full_lifecycle() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    // A shell that stays alive so write/resize/get_output run against a live PTY.
    let sh = write_fake_bin(dir.path(), "fakeshell", "while true; do sleep 1; done");
    let mut env = EnvRestore::new();
    env.set("SHELL", &sh.display().to_string());

    let work = tempfile::tempdir().unwrap();
    let h = start_web_pty_manager();

    let out = h
        .spawn_shell("term1".into(), 24, 80, work.path().to_path_buf())
        .await;
    assert!(out.is_ok(), "spawn_shell should succeed: {out:?}");

    assert_eq!(h.list().await, vec!["term1".to_string()]);
    assert!(h.write("term1", b"echo hi\n".to_vec()).await, "write");
    assert!(h.resize("term1", 40, 120).await, "resize");
    assert!(h.get_output("term1").await.is_some(), "get_output");
    assert!(h.kill("term1").await, "kill");
    assert!(h.list().await.is_empty(), "list empty after kill");
}

#[tokio::test]
async fn spawn_shell_error_arm_reports_err() {
    let _g = env_lock();
    let mut env = EnvRestore::new();
    env.set("SHELL", "/nonexistent/opman-fakeshell-zzz");
    let work = tempfile::tempdir().unwrap();
    let h = start_web_pty_manager();
    let res = h
        .spawn_shell("bad".into(), 24, 80, work.path().to_path_buf())
        .await;
    assert!(res.is_err(), "missing shell -> Err arm");
    assert!(h.list().await.is_empty());
}

#[tokio::test]
async fn all_spawn_arms_insert_ptys() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    for name in ["nvim", "gitui", "opencode", "claude"] {
        write_fake_bin(dir.path(), name, "exit 0");
    }
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1".to_string());
    let mut env = EnvRestore::new();
    env.remove("OPMAN_CLAUDE_BIN"); // fall back to PATH "claude"
    env.prepend_path(dir.path());

    let work_dir = tempfile::tempdir().unwrap();
    let work = work_dir.path().to_path_buf();
    let h = start_web_pty_manager();

    assert!(h
        .spawn_neovim("nv".into(), 24, 80, work.clone())
        .await
        .is_ok());
    assert!(h
        .spawn_gitui("gt".into(), 24, 80, work.clone())
        .await
        .is_ok());
    assert!(h
        .spawn_opencode("oc".into(), 24, 80, work.clone(), Some("s1".into()))
        .await
        .is_ok());
    assert!(h
        .spawn_claude_attach("cl".into(), 24, 80, work.clone(), "short".into())
        .await
        .is_ok());

    let mut ids = h.list().await;
    ids.sort();
    assert_eq!(
        ids,
        vec![
            "cl".to_string(),
            "gt".to_string(),
            "nv".to_string(),
            "oc".to_string()
        ]
    );

    // Dropping the handle closes the channel; the manager drains & kills all PTYs.
    drop(h);
}

// ── Foreground activity ─────────────────────────────────────────────

/// Wait until `predicate` holds for the PTY's activity, or give up after ~3s.
async fn await_activity(h: &WebPtyHandle, id: &str, want: PtyActivity) -> PtyActivity {
    let mut last = PtyActivity::Idle;
    for _ in 0..60 {
        last = h.activity(id).await.unwrap_or_default();
        if last == want {
            return last;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    last
}

#[tokio::test]
async fn activity_reports_running_while_a_command_holds_the_terminal() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    // `set -m` turns on job control, which is what puts a foreground command in
    // its own process group — the thing the classifier reads. A real terminal
    // gets this for free by running an interactive shell.
    let sh = write_fake_bin(
        dir.path(),
        "jobshell",
        "set -m\nsleep 30\nwhile true; do sleep 1; done",
    );
    let mut env = EnvRestore::new();
    env.set("SHELL", &sh.display().to_string());

    let h = start_web_pty_manager();
    h.spawn_shell("busy-term".into(), 24, 80, dir.path().to_path_buf())
        .await
        .expect("fake shell spawns");

    assert_eq!(
        await_activity(&h, "busy-term", PtyActivity::Running).await,
        PtyActivity::Running,
        "a foreground command should read as running"
    );
    assert!(h.kill("busy-term").await);
}

#[tokio::test]
async fn activity_is_idle_when_the_shell_owns_the_terminal() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    // No job control and no child: the script itself stays in the foreground.
    let sh = write_fake_bin(dir.path(), "idleshell", "while true; do sleep 1; done");
    let mut env = EnvRestore::new();
    env.set("SHELL", &sh.display().to_string());

    let h = start_web_pty_manager();
    h.spawn_shell("idle-term".into(), 24, 80, dir.path().to_path_buf())
        .await
        .expect("fake shell spawns");

    assert_eq!(
        h.activity("idle-term").await,
        Some(PtyActivity::Idle),
        "the spawned program owning the terminal is not work"
    );
    assert!(h.kill("idle-term").await);
}

#[tokio::test]
async fn activity_of_an_unknown_pty_is_none() {
    let h = start_web_pty_manager();
    assert_eq!(h.activity("nope").await, None);
}
