//! Requests waiting on a human.
//!
//! An agent asking `session/request_permission` is blocked on the answer, so opman parks the
//! request here and the reply route resolves it. Split from [`super::state`], which is about
//! sessions, because the thing that makes this more than a map is cancellation: the waiter
//! carries the session it belongs to, so aborting a turn can answer every prompt that turn
//! left on screen instead of leaving them to time out an hour later against an agent that
//! unwound long ago.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::oneshot;

use super::AcpEngine;
use crate::claude_engine::PendingReply;

/// One question opman is holding open, and whose turn it belongs to.
pub struct Pending {
    session: String,
    reply: oneshot::Sender<PendingReply>,
}

/// The parked requests for one agent.
#[derive(Default)]
pub struct Pendings(Mutex<HashMap<String, Pending>>);

impl AcpEngine {
    pub fn register_pending(&self, id: &str, session: &str) -> oneshot::Receiver<PendingReply> {
        let (reply, rx) = oneshot::channel();
        if let Ok(mut all) = self.pending.0.lock() {
            all.insert(
                id.to_string(),
                Pending {
                    session: session.to_string(),
                    reply,
                },
            );
        }
        rx
    }

    /// Answer one request. Returns whether *this* engine was the one holding it: a reply is
    /// fanned out across every engine, and an unconditional yes would stop the fan-out at the
    /// first engine asked rather than the one that asked the question.
    pub fn resolve_pending(&self, id: &str, reply: PendingReply) -> bool {
        let found = self
            .pending
            .0
            .lock()
            .ok()
            .and_then(|mut all| all.remove(id));
        found
            .map(|pending| pending.reply.send(reply).is_ok())
            .unwrap_or(false)
    }

    /// Reject everything a session still has open, returning the ids so the caller can tell
    /// the clients to drop the cards. Used by abort: ACP cancels the agent's outstanding
    /// permission requests along with the turn, so an unanswered prompt has nothing left to
    /// answer and leaving it up invites the user to reply into a turn that already ended.
    pub fn clear_session_pending(&self, session: &str) -> Vec<String> {
        let Ok(mut all) = self.pending.0.lock() else {
            return Vec::new();
        };
        let cancelled: Vec<String> = all
            .iter()
            .filter(|(_, pending)| pending.session == session)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &cancelled {
            if let Some(pending) = all.remove(id) {
                let _ = pending.reply.send(PendingReply::Reject);
            }
        }
        cancelled
    }
}

#[cfg(test)]
#[path = "pending_tests.rs"]
mod pending_tests;
