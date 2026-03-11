use std::time::Instant;

use tracing::{info, warn};

use super::super::types::*;

impl super::WebStateHandle {
    // ── Watcher management ──────────────────────────────────────

    /// Create or update a watcher for a session.
    pub async fn create_watcher(&self, req: WatcherConfigRequest) -> WatcherConfigResponse {
        let config = super::WatcherConfigInternal {
            session_id: req.session_id.clone(),
            project_idx: req.project_idx,
            idle_timeout_secs: req.idle_timeout_secs,
            continuation_message: req.continuation_message.clone(),
            include_original: req.include_original,
            original_message: req.original_message.clone(),
            hang_message: req.hang_message.clone(),
            hang_timeout_secs: req.hang_timeout_secs,
        };

        let mut inner = self.inner.write().await;
        inner.session_watchers.insert(req.session_id.clone(), config);

        // Determine current status
        let is_busy = inner.busy_sessions.contains(&req.session_id);
        let status = if is_busy { "running" } else { "waiting" }.to_string();

        drop(inner);

        let _ = self.event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
            session_id: req.session_id.clone(),
            action: "created".to_string(),
            idle_since_secs: None,
        }));

        info!(session_id = %req.session_id, "Watcher created");

        WatcherConfigResponse {
            session_id: req.session_id,
            project_idx: req.project_idx,
            idle_timeout_secs: req.idle_timeout_secs,
            continuation_message: req.continuation_message,
            include_original: req.include_original,
            original_message: req.original_message,
            hang_message: req.hang_message,
            hang_timeout_secs: req.hang_timeout_secs,
            status,
            idle_since_secs: None,
        }
    }

    /// Delete a watcher for a session.
    pub async fn delete_watcher(&self, session_id: &str) -> bool {
        let mut inner = self.inner.write().await;
        let removed = inner.session_watchers.remove(session_id).is_some();
        if removed {
            // Cancel any pending timer
            if let Some(handle) = inner.watcher_pending.remove(session_id) {
                handle.abort();
            }
            inner.watcher_idle_since.remove(session_id);

            drop(inner);

            let _ = self.event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
                session_id: session_id.to_string(),
                action: "deleted".to_string(),
                idle_since_secs: None,
            }));

            info!(session_id = %session_id, "Watcher deleted");
        }
        removed
    }

    /// List all active watchers with their current status.
    pub async fn list_watchers(&self) -> Vec<WatcherListEntry> {
        let inner = self.inner.read().await;
        let mut entries = Vec::new();

        for (sid, config) in &inner.session_watchers {
            let is_busy = inner.busy_sessions.contains(sid);
            let idle_since = inner.watcher_idle_since.get(sid);

            let (status, idle_since_secs) = if is_busy {
                ("running".to_string(), None)
            } else if let Some(since) = idle_since {
                let elapsed = since.elapsed().as_secs();
                ("idle_countdown".to_string(), Some(elapsed))
            } else {
                ("waiting".to_string(), None)
            };

            // Find session title and project name
            let mut title = sid.clone();
            let mut project_name = String::new();
            if let Some(project) = inner.projects.get(config.project_idx) {
                project_name = project.name.clone();
                if let Some(session) = project.sessions.iter().find(|s| s.id == *sid) {
                    title = session.title.clone();
                }
            }

            entries.push(WatcherListEntry {
                session_id: sid.clone(),
                session_title: title,
                project_name,
                idle_timeout_secs: config.idle_timeout_secs,
                status,
                idle_since_secs,
            });
        }

        entries
    }

    /// Get watcher config for a specific session.
    pub async fn get_watcher(&self, session_id: &str) -> Option<WatcherConfigResponse> {
        let inner = self.inner.read().await;
        let config = inner.session_watchers.get(session_id)?;
        let is_busy = inner.busy_sessions.contains(session_id);
        let idle_since = inner.watcher_idle_since.get(session_id);

        let (status, idle_since_secs) = if is_busy {
            ("running".to_string(), None)
        } else if let Some(since) = idle_since {
            let elapsed = since.elapsed().as_secs();
            ("idle_countdown".to_string(), Some(elapsed))
        } else {
            ("waiting".to_string(), None)
        };

        Some(WatcherConfigResponse {
            session_id: config.session_id.clone(),
            project_idx: config.project_idx,
            idle_timeout_secs: config.idle_timeout_secs,
            continuation_message: config.continuation_message.clone(),
            include_original: config.include_original,
            original_message: config.original_message.clone(),
            hang_message: config.hang_message.clone(),
            hang_timeout_secs: config.hang_timeout_secs,
            status,
            idle_since_secs,
        })
    }

    /// Get all sessions formatted for the watcher session picker.
    pub async fn get_watcher_sessions(&self) -> Vec<WatcherSessionEntry> {
        let inner = self.inner.read().await;
        let mut entries = Vec::new();

        for (idx, project) in inner.projects.iter().enumerate() {
            for session in &project.sessions {
                let is_current = project.active_session.as_deref() == Some(&session.id)
                    && idx == inner.active_project;
                let is_active = inner.busy_sessions.contains(&session.id);
                let has_watcher = inner.session_watchers.contains_key(&session.id);

                entries.push(WatcherSessionEntry {
                    session_id: session.id.clone(),
                    title: session.title.clone(),
                    project_name: project.name.clone(),
                    project_idx: idx,
                    is_current,
                    is_active,
                    has_watcher,
                });
            }
        }

        entries
    }

    /// Try to trigger a watcher when a session goes idle.
    /// Called from the SSE handler when `session.status` → `idle`.
    pub(super) async fn try_trigger_watcher(&self, session_id: &str) {
        let inner = self.inner.read().await;
        let watcher = match inner.session_watchers.get(session_id) {
            Some(w) => w.clone(),
            None => return,
        };

        // Check for active children (suppress if subagent sessions still running)
        let has_active_children = inner
            .session_children
            .get(session_id)
            .map(|children| children.iter().any(|c| inner.busy_sessions.contains(c)))
            .unwrap_or(false);

        drop(inner);

        if has_active_children {
            info!(
                session_id = %session_id,
                "Watcher: suppressed — subagent sessions still active"
            );
            let mut inner = self.inner.write().await;
            inner.watcher_idle_since.remove(session_id);
            return;
        }

        // Cancel any existing pending timer
        {
            let mut inner = self.inner.write().await;
            if let Some(prev_handle) = inner.watcher_pending.remove(session_id) {
                prev_handle.abort();
            }
            // Record when idle countdown started
            inner.watcher_idle_since.insert(session_id.to_string(), Instant::now());
        }

        let _ = self.event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
            session_id: session_id.to_string(),
            action: "countdown".to_string(),
            idle_since_secs: Some(0),
        }));

        let timeout = watcher.idle_timeout_secs;
        let msg = watcher.continuation_message.clone();
        let original = if watcher.include_original {
            watcher.original_message.clone()
        } else {
            None
        };
        let api = crate::api::ApiClient::new();
        let base_url = crate::app::base_url().to_string();

        let project_dir = {
            let inner = self.inner.read().await;
            inner
                .projects
                .get(watcher.project_idx)
                .map(|p| p.path.display().to_string())
                .unwrap_or_default()
        };

        let sid = session_id.to_string();
        let event_tx = self.event_tx.clone();

        info!(
            session_id = %sid,
            timeout_secs = timeout,
            "Watcher: scheduling continuation message after idle"
        );

        let handle = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(timeout)).await;
            info!(session_id = %sid, "Watcher: sending continuation message");
            let mut full_msg = String::new();
            if let Some(orig) = original {
                full_msg.push_str(&format!("[Original message]: {}\n\n", orig));
            }
            full_msg.push_str(&msg);
            if let Err(e) = api
                .send_system_message_async(&base_url, &project_dir, &sid, &full_msg)
                .await
            {
                warn!("Watcher failed to send message to {}: {}", sid, e);
            }
            let _ = event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
                session_id: sid.clone(),
                action: "triggered".to_string(),
                idle_since_secs: None,
            }));
        });

        let mut inner = self.inner.write().await;
        inner.watcher_pending.insert(session_id.to_string(), handle.abort_handle());
    }

    /// Cancel a pending watcher timer (called when session goes busy).
    pub(super) async fn cancel_watcher_timer(&self, session_id: &str) {
        let mut inner = self.inner.write().await;
        if inner.session_watchers.contains_key(session_id) {
            if let Some(handle) = inner.watcher_pending.remove(session_id) {
                handle.abort();
            }
            inner.watcher_idle_since.remove(session_id);

            drop(inner);

            let _ = self.event_tx.send(WebEvent::WatcherStatusChanged(WatcherStatusEvent {
                session_id: session_id.to_string(),
                action: "cancelled".to_string(),
                idle_since_secs: None,
            }));
        }
    }
}
