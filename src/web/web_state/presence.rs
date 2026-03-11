use super::super::types::*;

impl super::WebStateHandle {
    // ── Session Continuity: Presence + Activity ─────────────────────

    /// Register or update a client's presence.
    pub async fn register_presence(&self, req: &ClientPresence) {
        let mut state = self.inner.write().await;
        state.connected_clients.insert(req.client_id.clone(), req.clone());
        let snapshot = PresenceSnapshot {
            clients: state.connected_clients.values().cloned().collect(),
        };
        drop(state);
        let _ = self.event_tx.send(WebEvent::PresenceChanged(snapshot));
    }

    /// Remove a client's presence.
    pub async fn deregister_presence(&self, client_id: &str) {
        let mut state = self.inner.write().await;
        state.connected_clients.remove(client_id);
        let snapshot = PresenceSnapshot {
            clients: state.connected_clients.values().cloned().collect(),
        };
        drop(state);
        let _ = self.event_tx.send(WebEvent::PresenceChanged(snapshot));
    }

    /// Get current presence snapshot.
    pub async fn get_presence(&self) -> PresenceSnapshot {
        let state = self.inner.read().await;
        PresenceSnapshot {
            clients: state.connected_clients.values().cloned().collect(),
        }
    }

    /// Push an activity event for a session (stores + broadcasts).
    pub async fn push_activity_event(&self, event: ActivityEventPayload) {
        let session_id = event.session_id.clone();
        {
            let mut state = self.inner.write().await;
            let log = state.activity_log.entry(session_id).or_default();
            log.push(event.clone());
            // Ring buffer: keep last 200 events per session
            if log.len() > 200 {
                let drain = log.len() - 200;
                log.drain(..drain);
            }
        }
        let _ = self.event_tx.send(WebEvent::ActivityEvent(event));
    }

    /// Get recent activity events for a session.
    pub async fn get_activity_feed(&self, session_id: &str) -> Vec<ActivityEventPayload> {
        let state = self.inner.read().await;
        state
            .activity_log
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }
}
