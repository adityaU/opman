//! The lock-error arms of the registry mutators/accessors. A helper poisons a
//! specific `Mutex` (by panicking a thread while it holds the guard) so the
//! `.lock()` calls return `Err`, exercising every `Err(_) => …` / `.ok()?` /
//! `if let Ok(..)` fallback that the happy-path tests can't reach.

use super::*;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

/// Poison `engine.sessions` (and it stays poisoned for the rest of the test).
fn poison_sessions(e: &Arc<ClaudePEngine>) {
    let e2 = e.clone();
    let _ = std::thread::spawn(move || {
        let _g = e2.sessions.lock().unwrap();
        panic!("intentional poison");
    })
    .join();
    assert!(e.sessions.is_poisoned());
}

#[test]
fn poisoned_sessions_read_accessors_return_defaults() {
    let e = engine();
    poison_sessions(&e);
    assert!(e.get_session("x").is_none());
    assert!(e.list_for_dir("d").is_empty());
    assert!(e.busy_map().is_empty());
    assert!(e.session_id_for_claude_uuid("u").is_none());
    assert!(e.resume_uuid("x").is_none());
    assert!(!e.is_always_allowed("x", "Bash"));
    // effective_mode falls back to the engine default when the session read fails.
    assert_eq!(e.effective_mode("x"), "bypassPermissions");
}

#[test]
fn poisoned_sessions_mutators_are_noops() {
    let e = engine();
    poison_sessions(&e);
    // None of these may panic; each hits its lock-error early-return.
    e.ensure_subagent_session("p", "a", "", "d"); // Err(_) => return
    e.set_title("x", "t", true); // Err(_) => return
    assert!(!e.set_busy("x", true)); // Err(_) => return false
    e.set_claude_uuid("x", "u"); // get_session None → change guard false
    e.forget_claude_uuid("x"); // mutate None + save skip
    e.set_model("x", "m"); // mutate None + save skip
    e.set_agent("x", "a"); // resolve_agent (poisoned reads) + mutate None
    e.set_permission_mode("x", "plan"); // mutate None → dir None → no toast
    e.add_allowed_tool("x", "Bash"); // mutate None
    e.rename_session("x", "t"); // → set_title early return
                                // resolve_agent with poisoned session read still resolves against no cache.
    assert_eq!(e.resolve_agent("x", "plan"), "Plan");
}

#[tokio::test]
async fn poisoned_sessions_delete_session_is_noop() {
    let e = engine();
    poison_sessions(&e);
    // abort has no live proc; the remove() under poison yields None → no emit.
    e.delete_session("x").await;
    assert!(e.sessions.is_poisoned());
}

#[test]
fn poisoned_command_cache_init_accessors() {
    let e = engine();
    let e2 = e.clone();
    let _ = std::thread::spawn(move || {
        let _g = e2.command_cache.lock().unwrap();
        panic!("intentional poison");
    })
    .join();
    assert!(e.command_cache.is_poisoned());
    assert!(e.cached_init("d").is_none()); // .ok()? → None
    e.set_cached_init(
        "d",
        crate::claude_engine::claude_cli::InitInfo {
            commands: vec![],
            agents: vec![],
        },
    ); // if let Ok skip
    assert!(e.cached_init("d").is_none());
}

#[test]
fn poisoned_pending_register_and_resolve() {
    let e = engine();
    let e2 = e.clone();
    let _ = std::thread::spawn(move || {
        let _g = e2.pending.lock().unwrap();
        panic!("intentional poison");
    })
    .join();
    assert!(e.pending.is_poisoned());
    // register still returns a receiver even though the insert is skipped.
    let _rx = e.register_pending("req");
    // resolve finds nothing (lock err → None) → false.
    assert!(!e.resolve_pending("req", crate::claude_engine::PendingReply::Reject));
}
