//! Wave-2 tests that actually open a PTY and launch a harmless FAKE program for
//! each `spawn_*_pty`, driving the openpty + reader-thread + WebPty-construction
//! path. Fakes exit immediately (or print a line then exit) so nothing lingers.

use super::*;
use crate::web::pty_manager::manager::pty_test_support::{env_lock, write_fake_bin, EnvRestore};

/// Let the detached reader thread run, then kill the child.
fn drain_and_kill(mut pty: WebPty) {
    std::thread::sleep(std::time::Duration::from_millis(60));
    let _ = pty.child.kill();
    let _ = pty.output.drain_new();
}

#[test]
fn spawn_shell_pty_success_with_fake_shell() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    let sh = write_fake_bin(dir.path(), "fakeshell", "printf 'hi from shell\\n'\nexit 0");
    let mut env = EnvRestore::new();
    env.set("SHELL", &sh.display().to_string());

    let work = tempfile::tempdir().unwrap();
    let pty = spawn_shell_pty(24, 80, work.path()).expect("shell pty");
    assert_eq!(pty.rows, 24);
    assert_eq!(pty.cols, 80);
    drain_and_kill(pty);
}

#[test]
fn spawn_shell_pty_error_when_shell_missing() {
    let _g = env_lock();
    let mut env = EnvRestore::new();
    // Absolute, nonexistent shell -> spawn_command fails.
    env.set("SHELL", "/nonexistent/opman-fakeshell-xyz");
    let work = tempfile::tempdir().unwrap();
    assert!(spawn_shell_pty(24, 80, work.path()).is_err());
}

#[test]
fn spawn_neovim_pty_success_with_fake_nvim() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(dir.path(), "nvim", "exit 0");
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let work = tempfile::tempdir().unwrap();
    let pty = spawn_neovim_pty(30, 100, work.path()).expect("nvim pty");
    assert_eq!(pty.rows, 30);
    drain_and_kill(pty);
}

#[test]
fn spawn_neovim_pty_error_when_absent() {
    let _g = env_lock();
    let empty = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.set("PATH", &empty.path().display().to_string());
    let work = tempfile::tempdir().unwrap();
    assert!(spawn_neovim_pty(30, 100, work.path()).is_err());
}

#[test]
fn spawn_gitui_pty_success_with_fake_gitui() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(dir.path(), "gitui", "printf 'gitui\\n'\nexit 0");
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let work = tempfile::tempdir().unwrap();
    let pty = spawn_gitui_pty(20, 60, work.path()).expect("gitui pty");
    assert_eq!(pty.cols, 60);
    drain_and_kill(pty);
}

#[test]
fn spawn_gitui_pty_error_when_absent() {
    let _g = env_lock();
    let empty = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.set("PATH", &empty.path().display().to_string());
    let work = tempfile::tempdir().unwrap();
    assert!(spawn_gitui_pty(20, 60, work.path()).is_err());
}

#[test]
fn spawn_opencode_pty_success_new_and_existing_session() {
    let _g = env_lock();
    // base_url() must be initialized or it panics; set once (ignored if present).
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1".to_string());
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(dir.path(), "opencode", "exit 0");
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let work = tempfile::tempdir().unwrap();
    // No session id.
    let pty = spawn_opencode_pty(24, 80, work.path(), None).expect("opencode pty");
    drain_and_kill(pty);
    // With a session id (drives the `--session` arg branch).
    let pty2 = spawn_opencode_pty(24, 80, work.path(), Some("sess-1")).expect("opencode pty2");
    drain_and_kill(pty2);
}

#[test]
fn spawn_opencode_pty_error_when_absent() {
    let _g = env_lock();
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1".to_string());
    let empty = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.set("PATH", &empty.path().display().to_string());
    let work = tempfile::tempdir().unwrap();
    assert!(spawn_opencode_pty(24, 80, work.path(), None).is_err());
}

#[test]
fn spawn_claude_attach_pty_success_via_env_bin() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    let claude = write_fake_bin(dir.path(), "fakeclaude", "exit 0");
    let mut env = EnvRestore::new();
    env.set("OPMAN_CLAUDE_BIN", &claude.display().to_string());

    let work = tempfile::tempdir().unwrap();
    let pty = spawn_claude_attach_pty(24, 80, work.path(), "short-abc").expect("claude pty");
    drain_and_kill(pty);
}

#[test]
fn spawn_claude_attach_pty_success_via_path_default() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(dir.path(), "claude", "exit 0");
    let mut env = EnvRestore::new();
    // Unset OPMAN_CLAUDE_BIN so it falls back to "claude" resolved on PATH.
    env.remove("OPMAN_CLAUDE_BIN");
    env.prepend_path(dir.path());

    let work = tempfile::tempdir().unwrap();
    let pty = spawn_claude_attach_pty(24, 80, work.path(), "short-def").expect("claude pty default");
    drain_and_kill(pty);
}

#[test]
fn spawn_claude_attach_pty_error_when_absent() {
    let _g = env_lock();
    let empty = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.remove("OPMAN_CLAUDE_BIN");
    env.set("PATH", &empty.path().display().to_string());
    let work = tempfile::tempdir().unwrap();
    assert!(spawn_claude_attach_pty(24, 80, work.path(), "x").is_err());
}
