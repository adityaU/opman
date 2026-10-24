//! Session instruction delivery state.

use super::WebStateHandle;

impl WebStateHandle {
    /// Record that a session has been given its session instructions.
    ///
    /// Returns `true` the first time it is called for a session, so the caller
    /// can claim delivery and inject in the same step without a second lookup.
    pub async fn claim_instructions_delivery(&self, session_id: &str) -> bool {
        let mut inner = self.inner.write().await;
        inner.instructions_sent.insert(session_id.to_string())
    }

    /// Release a delivery claim after the runner rejects the opening turn.
    pub async fn release_instructions_delivery(&self, session_id: &str) {
        let mut inner = self.inner.write().await;
        inner.instructions_sent.remove(session_id);
    }

    /// Mark a session as already instructed without claiming a delivery.
    ///
    /// Used for a session discovered mid-conversation, and for the session a
    /// handoff produced (its opening message already carried the block).
    pub async fn mark_instructions_delivered(&self, session_id: &str) {
        let mut inner = self.inner.write().await;
        inner.instructions_sent.insert(session_id.to_string());
    }
}
