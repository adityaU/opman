use super::*;

use std::sync::Arc;

use crate::acp_engine::config::AgentConfig;

fn engine() -> Arc<AcpEngine> {
    let flags = crate::mcp_registry::BuiltinFlags::default();
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    Arc::new(AcpEngine::new(
        "test".to_string(),
        AgentConfig::default(),
        None,
        registry,
    ))
}

#[tokio::test]
async fn an_answer_reaches_the_waiting_request() {
    let engine = engine();
    let waiting = engine.register_pending("perm-1", "ses-1");
    assert!(engine.resolve_pending("perm-1", PendingReply::Permission("once".to_string())));
    assert!(matches!(
        waiting.await,
        Ok(PendingReply::Permission(reply)) if reply == "once"
    ));
}

/// A reply is fanned out across every engine, so the one that was not holding the request has
/// to say so — otherwise the fan-out stops at the first engine asked.
#[tokio::test]
async fn answering_a_request_this_engine_never_held_reports_false() {
    let engine = engine();
    assert!(!engine.resolve_pending("perm-unknown", PendingReply::Reject));
}

#[tokio::test]
async fn a_request_is_only_answered_once() {
    let engine = engine();
    let _waiting = engine.register_pending("perm-1", "ses-1");
    assert!(engine.resolve_pending("perm-1", PendingReply::Reject));
    assert!(!engine.resolve_pending("perm-1", PendingReply::Reject));
}

/// The gap this session-tagging closes: aborting a turn used to leave its permission prompts
/// on screen for the full hour-long timeout, waiting on an agent that had already unwound.
#[tokio::test]
async fn cancelling_a_turn_answers_every_prompt_it_left_open() {
    let engine = engine();
    let first = engine.register_pending("perm-1", "ses-1");
    let second = engine.register_pending("perm-2", "ses-1");

    let mut cancelled = engine.clear_session_pending("ses-1");
    cancelled.sort();
    assert_eq!(cancelled, vec!["perm-1".to_string(), "perm-2".to_string()]);
    assert!(matches!(first.await, Ok(PendingReply::Reject)));
    assert!(matches!(second.await, Ok(PendingReply::Reject)));
}

/// One session's abort must not answer another's prompts: an engine drives every session it
/// has, and they run turns independently.
#[tokio::test]
async fn cancelling_one_turn_leaves_other_sessions_alone() {
    let engine = engine();
    let mine = engine.register_pending("perm-1", "ses-1");
    let theirs = engine.register_pending("perm-2", "ses-2");

    assert_eq!(engine.clear_session_pending("ses-1"), vec!["perm-1"]);
    assert!(matches!(mine.await, Ok(PendingReply::Reject)));

    // Still open, and still answerable.
    assert!(engine.resolve_pending("perm-2", PendingReply::Permission("always".to_string())));
    assert!(matches!(theirs.await, Ok(PendingReply::Permission(_))));
}

/// Aborting a session with nothing outstanding is the common case, and must be silent.
#[tokio::test]
async fn cancelling_with_nothing_open_returns_nothing() {
    let engine = engine();
    assert!(engine.clear_session_pending("ses-1").is_empty());
}
