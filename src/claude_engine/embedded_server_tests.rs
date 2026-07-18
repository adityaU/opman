//! Coverage for `start_embedded_server` — the adapter bootstrap that mints the engine,
//! sets the global `ENGINE`, binds a loopback port, and spawns the model-fetch /
//! status-poller / reaper background tasks. Driven with a temp config dir and a stub
//! `claude` binary (`echo`) so nothing touches the real home/config or a live CLI. The
//! detached background tasks are cancelled when this test's tokio runtime is dropped.
use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;

#[tokio::test]
async fn start_embedded_server_binds_and_returns_url() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Isolate persistence + the locate glob roots to a throwaway dir, and stub the
    // `claude` binary so the model-fetch / poller / reaper spawns are inert.
    let tmp = tempfile::tempdir().unwrap();
    let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let prev_home = std::env::var_os("HOME");
    let prev_bin = std::env::var("OPMAN_CLAUDE_BIN").ok();
    std::env::set_var("XDG_CONFIG_HOME", tmp.path());
    std::env::set_var("HOME", tmp.path());
    std::env::set_var("OPMAN_CLAUDE_BIN", "echo");

    let (url, handle) = start_embedded_server((false, false, false, false))
        .await
        .expect("embedded server starts");

    // Loopback URL was bound and stashed on the (global) engine.
    assert!(url.starts_with("http://127.0.0.1:"));
    assert!(url.trim_end_matches(|c: char| c.is_ascii_digit()).ends_with(':'));
    // The embedded adapter manages no external child process → a None-holding handle.
    assert!(handle.lock().unwrap().is_none());
    // The global accessor resolves an engine (this call sets it if unset — another
    // test may have set it first, so only assert presence, not identity).
    assert!(super::engine().is_some());

    match prev_xdg {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
    match prev_home {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    match prev_bin {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
}
