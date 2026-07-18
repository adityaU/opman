//! The env-driven configuration branch of `ClaudePEngine::new`: a non-empty
//! `OPMAN_CLAUDE_PERMISSION_MODE` is adopted as the engine default mode (the
//! `.filter(nonempty)` keep-arm, complementing `mod_tests`' fallback case).

use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;

#[test]
fn new_adopts_nonempty_permission_mode_from_env() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("OPMAN_CLAUDE_PERMISSION_MODE").ok();

    std::env::set_var("OPMAN_CLAUDE_PERMISSION_MODE", "plan");
    let e = ClaudePEngine::new(None, (false, false, false, false));
    assert_eq!(e.default_mode, "plan");
    // A session with no explicit mode inherits the engine default.
    let s = e.create_session("d", "", "A");
    assert_eq!(e.effective_mode(&s.id), "plan");

    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_PERMISSION_MODE", v),
        None => std::env::remove_var("OPMAN_CLAUDE_PERMISSION_MODE"),
    }
}
