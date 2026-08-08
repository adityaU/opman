//! Per-turn bookkeeping: the folded transcript, replay and in-flight flags, and the
//! follow-up a user typed while the agent was busy.
//!
//! Split from [`super::state`], which owns the durable session registry. These live and die
//! with a turn; nothing here is persisted.

use serde_json::Value;

use super::attach::Prompt;
use super::{AcpEngine, Transcript};

impl AcpEngine {
    // ── transcripts ──────────────────────────────────────────────────
    /// Run `f` against a session's transcript, creating it on first use.
    pub fn with_transcript<T>(&self, id: &str, f: impl FnOnce(&mut Transcript) -> T) -> T
    where
        T: Default,
    {
        let Ok(mut all) = self.transcripts.lock() else {
            return T::default();
        };
        let transcript = all
            .entry(id.to_string())
            .or_insert_with(|| Transcript::new(id));
        f(transcript)
    }

    /// Rendered messages in the opencode `{info, parts}` shape.
    pub fn messages(&self, id: &str) -> Vec<Value> {
        self.transcripts
            .lock()
            .ok()
            .and_then(|all| {
                all.get(id)
                    .map(|t| t.messages().iter().map(|m| m.to_value()).collect())
            })
            .unwrap_or_default()
    }

    /// Hand a session's history back to the agent for a `session/load` replay.
    pub fn begin_replay(&self, id: &str) {
        self.set_replaying(id, true);
        if let Ok(mut all) = self.transcripts.lock() {
            if let Some(transcript) = all.get_mut(id) {
                transcript.begin_replay();
            }
        }
    }

    /// Close a replay and return what changed, so the caller can broadcast it.
    pub fn end_replay(&self, id: &str) -> Vec<super::emit::Emit> {
        self.set_replaying(id, false);
        self.with_transcript(id, |t| {
            let mut out = Vec::new();
            t.end_replay(&mut out);
            out
        })
    }

    pub fn set_replaying(&self, id: &str, replaying: bool) {
        if let Ok(mut map) = self.replaying.lock() {
            map.insert(id.to_string(), replaying);
        }
    }

    pub fn is_replaying(&self, id: &str) -> bool {
        self.replaying
            .lock()
            .map(|m| m.get(id).copied().unwrap_or(false))
            .unwrap_or(false)
    }

    pub fn mark_inflight(&self, id: &str, inflight: bool) {
        if let Ok(mut map) = self.inflight.lock() {
            map.insert(id.to_string(), inflight);
        }
    }

    pub fn has_inflight(&self, id: &str) -> bool {
        self.inflight
            .lock()
            .map(|m| m.get(id).copied().unwrap_or(false))
            .unwrap_or(false)
    }

    /// Hold a follow-up until the running turn ends. A newer one replaces the pending one:
    /// the user's latest intent is the one worth sending.
    pub fn queue_followup(&self, id: &str, prompt: Prompt) {
        if let Ok(mut map) = self.followups.lock() {
            map.insert(id.to_string(), prompt);
        }
    }

    pub fn take_followup(&self, id: &str) -> Option<Prompt> {
        self.followups.lock().ok()?.remove(id)
    }
}
