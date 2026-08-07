use super::super::types::*;

/// Clients that haven't sent a heartbeat in this many seconds are considered stale.
const STALE_CLIENT_TIMEOUT_SECS: u64 = 120;

impl super::WebStateHandle {
    // ── Session Continuity: Presence ────────────────────────────────

    /// Register or update a client's presence.
    pub async fn register_presence(&self, req: &ClientPresence) {
        let mut state = self.inner.write().await;
        state
            .connected_clients
            .insert(req.client_id.clone(), req.clone());
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

    /// Evict clients whose `last_seen` timestamp is older than `STALE_CLIENT_TIMEOUT_SECS`.
    /// Should be called periodically (e.g. every 60s from a background task).
    pub async fn evict_stale_clients(&self) {
        let now = chrono::Utc::now();
        let threshold = now - chrono::Duration::seconds(STALE_CLIENT_TIMEOUT_SECS as i64);

        let changed;
        {
            let mut state = self.inner.write().await;
            let before = state.connected_clients.len();
            state.connected_clients.retain(|_, c| {
                // Parse ISO 8601 last_seen; if unparseable, evict
                chrono::DateTime::parse_from_rfc3339(&c.last_seen)
                    .map(|dt| dt >= threshold)
                    .unwrap_or(false)
            });
            changed = state.connected_clients.len() != before;
        }

        if changed {
            let state = self.inner.read().await;
            let snapshot = PresenceSnapshot {
                clients: state.connected_clients.values().cloned().collect(),
            };
            drop(state);
            let _ = self.event_tx.send(WebEvent::PresenceChanged(snapshot));
        }
    }

    /// Spawn a background task that periodically evicts stale clients.
    pub(super) fn spawn_presence_cleanup(&self) {
        let handle = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                handle.evict_stale_clients().await;
            }
        });
    }
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod presence_tests;
