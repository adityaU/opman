//! Tests that actually open a PTY and launch a harmless FAKE program for each
//! kind, driving the openpty + reader-thread + WebPty-construction path. Fakes
//! exit immediately (or print a line then exit) so nothing lingers.

use super::*;
use super::super::kind::PtyKind;
use crate::web::pty_manager::manager::pty_test_support::{env_lock, write_fake_bin, EnvRestore};

/// Let the detached reader thread run, then kill the child.
fn drain_and_kill(mut pty: WebPty) {
    std::thread::sleep(std::time::Duration::from_millis(60));
    let _ = pty.child.kill();
    let _ = pty.output.drain_new();
}

/// Start one program in `project`, labelled as the manager would label it.
fn start(program: PtyProgram, project: &std::path::Path, rows: u16, cols: u16) -> Result<WebPty> {
    let kind = program.kind();
    let spec = SpawnSpec {
        id: "test".into(),
        program,
        project: project.to_path_buf(),
        label: None,
        rows,
        cols,
    };
    spawn_pty(&spec, format!("{} 1", kind.label()))
}

#[test]
fn shell_pty_success_with_fake_shell() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    let sh = write_fake_bin(dir.path(), "fakeshell", "printf 'hi from shell\\n'\nexit 0");
    let mut env = EnvRestore::new();
    env.set("SHELL", &sh.display().to_string());

    let work = tempfile::tempdir().unwrap();
    let pty = start(PtyProgram::Shell, work.path(), 24, 80).expect("shell pty");
    assert_eq!(pty.rows, 24);
    assert_eq!(pty.cols, 80);
    assert_eq!(pty.meta.kind, PtyKind::Shell);
    assert_eq!(pty.meta.label, "Shell 1");
    assert_eq!(pty.meta.project, work.path());
    drain_and_kill(pty);
}

#[test]
fn shell_pty_error_when_shell_missing() {
    let _g = env_lock();
    let mut env = EnvRestore::new();
    // Absolute, nonexistent shell -> spawn_command fails.
    env.set("SHELL", "/nonexistent/opman-fakeshell-xyz");
    let work = tempfile::tempdir().unwrap();
    assert!(start(PtyProgram::Shell, work.path(), 24, 80).is_err());
}

#[test]
fn neovim_pty_success_with_fake_nvim() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(dir.path(), "nvim", "exit 0");
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let work = tempfile::tempdir().unwrap();
    let pty = start(PtyProgram::Neovim, work.path(), 30, 100).expect("nvim pty");
    assert_eq!(pty.rows, 30);
    drain_and_kill(pty);
}

#[test]
fn neovim_pty_error_when_absent() {
    let _g = env_lock();
    let empty = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.set("PATH", &empty.path().display().to_string());
    let work = tempfile::tempdir().unwrap();
    assert!(start(PtyProgram::Neovim, work.path(), 30, 100).is_err());
}

#[test]
fn gitui_pty_success_with_fake_gitui() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(dir.path(), "gitui", "printf 'gitui\\n'\nexit 0");
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let work = tempfile::tempdir().unwrap();
    let pty = start(PtyProgram::Git, work.path(), 20, 60).expect("gitui pty");
    assert_eq!(pty.cols, 60);
    drain_and_kill(pty);
}

#[test]
fn gitui_pty_error_when_absent() {
    let _g = env_lock();
    let empty = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.set("PATH", &empty.path().display().to_string());
    let work = tempfile::tempdir().unwrap();
    assert!(start(PtyProgram::Git, work.path(), 20, 60).is_err());
}

#[test]
fn opencode_pty_success_new_and_existing_session() {
    let _g = env_lock();
    // base_url() must be initialized or it panics; set once (ignored if present).
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1".to_string());
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(dir.path(), "opencode", "exit 0");
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let work = tempfile::tempdir().unwrap();
    // No session id.
    let pty = start(PtyProgram::Opencode { session_id: None }, work.path(), 24, 80).expect("opencode pty");
    drain_and_kill(pty);
    // With a session id (drives the `--session` arg branch).
    let pty2 = start(PtyProgram::Opencode { session_id: Some("sess-1".into()) }, work.path(), 24, 80).expect("opencode pty2");
    drain_and_kill(pty2);
}

#[test]
fn opencode_pty_error_when_absent() {
    let _g = env_lock();
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1".to_string());
    let empty = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.set("PATH", &empty.path().display().to_string());
    let work = tempfile::tempdir().unwrap();
    assert!(start(PtyProgram::Opencode { session_id: None }, work.path(), 24, 80).is_err());
}

#[test]
fn claude_attach_pty_success_via_env_bin() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    let claude = write_fake_bin(dir.path(), "fakeclaude", "exit 0");
    let mut env = EnvRestore::new();
    env.set("OPMAN_CLAUDE_BIN", &claude.display().to_string());

    let work = tempfile::tempdir().unwrap();
    let pty = start(PtyProgram::ClaudeAttach { short_id: "short-abc".into() }, work.path(), 24, 80).expect("claude pty");
    drain_and_kill(pty);
}

#[test]
fn claude_attach_pty_success_via_path_default() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(dir.path(), "claude", "exit 0");
    let mut env = EnvRestore::new();
    // Unset OPMAN_CLAUDE_BIN so it falls back to "claude" resolved on PATH.
    env.remove("OPMAN_CLAUDE_BIN");
    env.prepend_path(dir.path());

    let work = tempfile::tempdir().unwrap();
    let pty =
        start(PtyProgram::ClaudeAttach { short_id: "short-def".into() }, work.path(), 24, 80).expect("claude pty default");
    drain_and_kill(pty);
}

#[test]
fn claude_attach_pty_error_when_absent() {
    let _g = env_lock();
    let empty = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.remove("OPMAN_CLAUDE_BIN");
    env.set("PATH", &empty.path().display().to_string());
    let work = tempfile::tempdir().unwrap();
    assert!(start(PtyProgram::ClaudeAttach { short_id: "x".into() }, work.path(), 24, 80).is_err());
}
