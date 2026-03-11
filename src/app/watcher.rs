use crate::app::App;

impl App {
    /// Try to trigger a watcher for the given session.
    ///
    /// Suppressed when `has_active_children` is true — the parent should not
    /// be considered fully idle while subagent sessions are still running.
    pub fn try_trigger_watcher(&mut self, session_id: &str, has_active_children: bool) {
        let watcher = match self.session_watchers.get(session_id) {
            Some(w) => w,
            None => return,
        };

        // Cancel any existing pending timer first (e.g. from a previous idle).
        if let Some(prev_handle) = self.watcher_pending.remove(session_id) {
            prev_handle.abort();
        }

        if has_active_children {
            tracing::info!(
                session_id = %session_id,
                "Watcher: suppressed — subagent sessions still active"
            );
            self.watcher_idle_since.remove(session_id);
            return;
        }

        let timeout = watcher.idle_timeout_secs;
        let msg = watcher.continuation_message.clone();
        let original = if watcher.include_original {
            watcher.original_message.clone()
        } else {
            None
        };
        let api = crate::api::ApiClient::new();
        let base_url = crate::app::base_url().to_string();
        let project_dir = self
            .projects
            .get(watcher.project_idx)
            .map(|p| p.path.display().to_string())
            .unwrap_or_default();
        let sid = session_id.to_string();
        tracing::info!(
            session_id = %sid,
            timeout_secs = timeout,
            "Watcher: scheduling continuation message after idle"
        );

        // Record when idle countdown started (for UI display).
        self.watcher_idle_since
            .insert(session_id.to_string(), std::time::Instant::now());

        let handle = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(timeout)).await;
            tracing::info!(session_id = %sid, "Watcher: sending continuation message");
            let mut full_msg = String::new();
            if let Some(orig) = original {
                full_msg.push_str(&format!("[Original message]: {}\n\n", orig));
            }
            full_msg.push_str(&msg);
            if let Err(e) = api
                .send_system_message_async(&base_url, &project_dir, &sid, &full_msg)
                .await
            {
                tracing::warn!("Watcher failed to send message to {}: {}", sid, e);
            }
        });
        self.watcher_pending
            .insert(session_id.to_string(), handle.abort_handle());
    }
}
