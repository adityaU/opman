//! Hydration gating: when a history read is allowed to spawn an agent, and when it must
//! not. Every case here was a way for opening an old session to go wrong — an empty
//! transcript, a respawn per poll, or a conversation silently replaced by a fresh one.

use super::*;
use crate::acp_engine::config::AgentConfig;

fn engine() -> Arc<AcpEngine> {
    Arc::new(AcpEngine::new(
        "test".to_string(),
        AgentConfig::default(),
        None,
        crate::mcp_registry::RegistryHandle::default(),
    ))
}

/// A session restored from disk: an agent session id, and nothing rendered.
fn cold_session(engine: &Arc<AcpEngine>, acp_session: Option<&str>) -> String {
    let session = engine.create_session("/tmp/project", "", "restored");
    if let Some(acp) = acp_session {
        engine.bind_acp_session(&session.id, acp);
    }
    session.id
}

/// The bug this module exists for: a persisted session has history on the agent's side and
/// none in memory, so a read has to go and get it.
#[tokio::test]
async fn a_restored_session_with_agent_history_is_replayed() {
    let engine = engine();
    engine.note_load_capable(true);
    let id = cold_session(&engine, Some("acp-1"));
    assert!(replayable(&engine, &id).await);
}

/// An agent that cannot `session/load` would answer a history read by starting a brand new
/// conversation and rebinding the stored id — losing exactly what was being asked for.
#[tokio::test]
async fn an_agent_that_cannot_load_is_never_asked_to() {
    let engine = engine();
    engine.note_load_capable(false);
    let id = cold_session(&engine, Some("acp-1"));
    assert!(!replayable(&engine, &id).await);
}

/// Unknown capability reads as no: the startup probe settles it within seconds, and
/// guessing yes means spawning a child to find out.
#[tokio::test]
async fn capability_is_assumed_absent_until_the_probe_answers() {
    let engine = engine();
    let id = cold_session(&engine, Some("acp-1"));
    assert!(!replayable(&engine, &id).await);
}

/// A session that has never reached an agent has nothing to replay; connecting would only
/// spawn a child for an empty conversation.
#[tokio::test]
async fn a_session_that_never_ran_is_left_alone() {
    let engine = engine();
    engine.note_load_capable(true);
    let id = cold_session(&engine, None);
    assert!(!replayable(&engine, &id).await);
}

/// The web UI polls the message list. Without a one-shot marker a session whose agent
/// fails to start would respawn it on every poll.
#[tokio::test]
async fn a_failed_attempt_is_not_retried_on_every_poll() {
    let engine = engine();
    engine.note_load_capable(true);
    let id = cold_session(&engine, Some("acp-1"));
    engine.mark_hydrated(&id);
    assert!(!replayable(&engine, &id).await);
    // Deleting the session forgets the attempt, so a reused id is not poisoned by it.
    engine.forget_hydrated(&id);
    assert!(replayable(&engine, &id).await);
}

/// Subagent rows are read from the agent's on-disk transcript and have no ACP session of
/// their own, so there is nothing to load.
#[tokio::test]
async fn subagent_rows_are_not_hydrated() {
    let engine = engine();
    engine.note_load_capable(true);
    let parent = cold_session(&engine, None);
    engine.ensure_subagent_session(&parent, "agent-1", "Explore", "/tmp/project");
    engine.bind_acp_session("agent-1", "acp-child");
    assert!(!replayable(&engine, "agent-1").await);
}

/// Messages already in memory are served as they are — a read of a live session must never
/// reach for the connection map.
#[tokio::test]
async fn a_warm_transcript_is_served_without_touching_the_agent() {
    let engine = engine();
    engine.note_load_capable(true);
    let id = cold_session(&engine, Some("acp-1"));
    engine.with_transcript(&id, |t| {
        let mut out = Vec::new();
        t.chunk(
            super::super::emit::Chunk::Text,
            Some("m1"),
            "hello",
            &mut out,
        );
    });
    assert_eq!(messages(&engine, &id).await.len(), 1);
    assert!(!engine.was_hydrated(&id));
}
