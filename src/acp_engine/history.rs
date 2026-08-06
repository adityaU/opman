//! Lazy history hydration: making an old session show its conversation again.
//!
//! The transcript is folded in memory and never written to disk, on the principle that the
//! agent already owns the conversation and a second copy is a second thing to keep in sync.
//! The consequence is that a session restored from `acp_*_sessions.json` has a title, a
//! directory and an agent session id — and no messages.
//!
//! ACP's only way to recover them is `session/load`, which replays the conversation over
//! `session/update` and so needs a live connection. That connection used to be established
//! by exactly one thing: sending a prompt. Opening a session from a previous run therefore
//! rendered an empty transcript until the user typed into it.
//!
//! So the first read of a cold session's messages connects, and the replay it triggers
//! fills the transcript before the response is written. Every guard below exists to keep
//! that from happening when it would cost a child process and return nothing.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tracing::{debug, warn};

use super::AcpEngine;

/// How long a history read waits for the agent to spawn, handshake and replay. A cold
/// `npx` agent takes a second or two and a long conversation a little more; past this it
/// is hanging, and an empty list the user can retry by reopening beats a request that
/// never answers.
const REPLAY_TIMEOUT: Duration = Duration::from_secs(45);

/// Rendered messages for a session, replaying the agent's history first when there is
/// history to replay and nothing in memory yet.
pub(super) async fn messages(engine: &Arc<AcpEngine>, id: &str) -> Vec<Value> {
    let rendered = engine.messages(id);
    if !rendered.is_empty() || !replayable(engine, id).await {
        return rendered;
    }
    hydrate(engine, id).await;
    engine.messages(id)
}

/// Connect so the agent replays. The attempt is recorded before it runs, not after: an
/// agent that fails to start must not be respawned by every poll of the message list.
async fn hydrate(engine: &Arc<AcpEngine>, id: &str) {
    engine.mark_hydrated(id);
    match tokio::time::timeout(REPLAY_TIMEOUT, engine.conns.ensure(engine, id)).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => debug!(session = %id, "acp history replay failed: {e}"),
        Err(_) => warn!(session = %id, "acp history replay timed out"),
    }
}

/// Whether connecting now would replay history, rather than cost a child process and
/// return nothing.
///
/// Gating on the agent's advertised `loadSession` is what keeps this honest: without it a
/// history read of an agent that cannot replay would spawn a child, create a *new* agent
/// session, and rebind the stored id — quietly discarding the conversation the user opened
/// the session to read.
async fn replayable(engine: &Arc<AcpEngine>, id: &str) -> bool {
    let Some(session) = engine.get_session(id) else {
        return false;
    };
    // Subagent rows are read from the agent's on-disk transcript and have no ACP session.
    if session.is_subagent || engine.was_hydrated(id) || !engine.load_capable() {
        return false;
    }
    if session.acp_session.unwrap_or_default().is_empty() {
        return false;
    }
    engine.conns.existing(id).await.is_none()
}

impl AcpEngine {
    /// Record what the agent's `initialize` reply said about resuming sessions. Answered
    /// once by the startup probe, then re-confirmed by every real handshake.
    pub(super) fn note_load_capable(&self, capable: bool) {
        if let Ok(mut known) = self.load_capable.lock() {
            *known = Some(capable);
        }
    }

    /// Whether the agent is known to support `session/load`. Unknown reads as `false`: the
    /// probe runs at startup and settles the question long before a user opens a session,
    /// and guessing yes would spawn a child to find out.
    pub(super) fn load_capable(&self) -> bool {
        self.load_capable
            .lock()
            .map(|known| known.unwrap_or(false))
            .unwrap_or(false)
    }

    pub(super) fn mark_hydrated(&self, id: &str) {
        if let Ok(mut done) = self.hydrated.lock() {
            done.insert(id.to_string());
        }
    }

    pub(super) fn was_hydrated(&self, id: &str) -> bool {
        self.hydrated
            .lock()
            .map(|done| done.contains(id))
            .unwrap_or(false)
    }

    pub(super) fn forget_hydrated(&self, id: &str) {
        if let Ok(mut done) = self.hydrated.lock() {
            done.remove(id);
        }
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod history_tests;
