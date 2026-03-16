use super::super::types::*;

/// Clients that haven't sent a heartbeat in this many seconds are considered stale.
const STALE_CLIENT_TIMEOUT_SECS: u64 = 120;

/// Maximum number of sessions that keep activity logs in memory.
/// Oldest sessions (by last event time) are pruned when this is exceeded.
const MAX_ACTIVITY_LOG_SESSIONS: usize = 50;

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

            // Prune activity logs if too many sessions are tracked
            if state.activity_log.len() > MAX_ACTIVITY_LOG_SESSIONS {
                // Find sessions with fewest recent events and remove them
                let mut sessions_by_size: Vec<(String, usize)> = state
                    .activity_log
                    .iter()
                    .map(|(k, v)| (k.clone(), v.len()))
                    .collect();
                sessions_by_size.sort_by_key(|(_, len)| *len);
                let to_remove = state.activity_log.len() - MAX_ACTIVITY_LOG_SESSIONS;
                for (sid, _) in sessions_by_size.into_iter().take(to_remove) {
                    state.activity_log.remove(&sid);
                }
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

    /// Remove activity log for a specific session (e.g. when session is deleted).
    pub async fn clear_activity_log(&self, session_id: &str) {
        let mut state = self.inner.write().await;
        state.activity_log.remove(session_id);
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
