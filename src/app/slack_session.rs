use crate::app::App;
use crate::config::ProjectEntry;
use tracing::info;

impl App {
    /// Handle `SlackBackgroundEvent::ConnectionStatus`.
    pub(super) fn handle_connection_status(
        &mut self,
        status: crate::slack::SlackConnectionStatus,
    ) {
        info!("Slack connection status: {:?}", status);

        // On first successful connection, ensure the triage project
        // exists in the runtime project list with SSE listeners and a session.
        if matches!(status, crate::slack::SlackConnectionStatus::Connected) {
            if let Ok(triage_dir) = crate::slack::triage_project_dir() {
                let _ = std::fs::create_dir_all(&triage_dir);
                let triage_canon =
                    std::fs::canonicalize(&triage_dir).unwrap_or_else(|_| triage_dir.clone());
                let already_present = self.projects.iter().any(|p| p.path == triage_canon);
                if !already_present {
                    info!("Auto-adding slack-triage project on first connection");
                    self.add_project(ProjectEntry {
                        name: "slack-triage".to_string(),
                        path: triage_canon.to_string_lossy().to_string(),
                        terminal_command: None,
                    });
                    let _ = self.config.save();
                }

                // Ensure SSE listener + session poller are running for the triage project.
                if let Some(triage_idx) =
                    self.projects.iter().position(|p| p.path == triage_canon)
                {
                    let dir_str = triage_canon.to_string_lossy().to_string();

                    // Spawn SSE listener and session poller if the project was
                    // just added (they wouldn't have been started at app boot).
                    if !already_present {
                        info!(
                            "Spawning SSE listener and session poller for slack-triage project (idx={})",
                            triage_idx
                        );
                        crate::sse::spawn_sse_listener(&self.bg_tx, triage_idx, dir_str.clone());
                        crate::sse::spawn_session_poller(
                            &self.bg_tx,
                            triage_idx,
                            dir_str.clone(),
                        );
                        crate::sse::spawn_provider_fetcher(&self.bg_tx, triage_idx, dir_str);
                    }

                    // If the triage project has no sessions, trigger new session creation.
                    if self.projects[triage_idx].sessions.is_empty() {
                        info!("Triggering new session for slack-triage project");
                        self.pending_new_session = Some(triage_idx);
                    }
                }
            }

            // Restore persisted session→thread mappings and re-attach watchers.
            self.restore_slack_session_map(&status);
        }

        if let Some(ref state) = self.slack_state {
            let st = state.clone();
            let is_reconnect = matches!(status, crate::slack::SlackConnectionStatus::Connected);
            tokio::spawn(async move {
                let mut s = st.lock().await;
                s.log(
                    crate::slack::SlackLogLevel::Info,
                    format!("Connection status: {:?}", status),
                );
                if is_reconnect
                    && !matches!(s.status, crate::slack::SlackConnectionStatus::Connected)
                {
                    s.metrics.reconnections += 1;
                }
                s.status = status;
            });
        }
        self.needs_redraw = true;
    }

    /// Restore persisted Slack session→thread mappings on reconnect.
    fn restore_slack_session_map(&self, status: &crate::slack::SlackConnectionStatus) {
        if !matches!(status, crate::slack::SlackConnectionStatus::Connected) {
            return;
        }
        if let Some(ref state) = self.slack_state {
            match crate::slack::SlackSessionMap::load() {
                Ok(map) => {
                    if !map.session_threads.is_empty() {
                        info!(
                            "Slack: restoring {} session mapping(s) from disk",
                            map.session_threads.len()
                        );
                        let project_paths: Vec<String> = self
                            .projects
                            .iter()
                            .map(|p| p.path.to_string_lossy().to_string())
                            .collect();
                        let bot_token = self
                            .slack_auth
                            .as_ref()
                            .map(|a| a.bot_token.clone())
                            .unwrap_or_default();
                        let buffer_secs = self.config.settings.slack.relay_buffer_secs;
                        let base_url = crate::app::base_url().to_string();
                        let st = state.clone();

                        tokio::spawn(async move {
                            let mut s = st.lock().await;
                            s.thread_sessions = map.thread_sessions;
                            s.session_threads = map.session_threads.clone();
                            s.session_msg_offset = map.msg_offsets;

                            for (session_id, (channel, thread_ts)) in &map.session_threads {
                                if s.relay_abort_handles.contains_key(session_id) {
                                    continue;
                                }
                                let project_dir = s
                                    .thread_sessions
                                    .values()
                                    .find(|(_, sid)| sid == session_id)
                                    .and_then(|(pidx, _)| project_paths.get(*pidx))
                                    .cloned()
                                    .unwrap_or_default();

                                if project_dir.is_empty() {
                                    tracing::warn!(
                                        "Slack: skipping watcher restore for session {} (project not found)",
                                        &session_id[..8.min(session_id.len())]
                                    );
                                    continue;
                                }

                                let handle = crate::slack::spawn_session_relay_watcher(
                                    session_id.clone(),
                                    project_dir,
                                    channel.clone(),
                                    thread_ts.clone(),
                                    bot_token.clone(),
                                    base_url.clone(),
                                    buffer_secs,
                                    st.clone(),
                                );
                                s.relay_abort_handles
                                    .insert(session_id.clone(), handle.abort_handle());
                                info!(
                                    "Slack: restored relay watcher for session {}",
                                    &session_id[..8.min(session_id.len())]
                                );
                            }
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("Slack: failed to load session map from disk: {}", e);
                }
            }
        }
    }

    /// Handle `SlackBackgroundEvent::ResponseBatch`.
    pub(super) fn handle_response_batch(
        &mut self,
        channel: String,
        thread_ts: String,
        text: String,
    ) {
        let slack_state = self.slack_state.clone();
        if let Some(ref auth) = self.slack_auth {
            let bot_token = auth.bot_token.clone();
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                let chunks = crate::slack::chunk_for_slack(&text, 39_000);
                for chunk in chunks {
                    let _ = crate::slack::post_message(
                        &client,
                        &bot_token,
                        &channel,
                        &chunk,
                        Some(&thread_ts),
                    )
                    .await;
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
                if let Some(ref state) = slack_state {
                    let mut s = state.lock().await;
                    s.metrics.batches_sent += 1;
                    s.log(
                        crate::slack::SlackLogLevel::Info,
                        format!("Response batch sent to thread {}", thread_ts),
                    );
                }
            });
        }
    }

    /// Handle `SlackBackgroundEvent::OAuthComplete`.
    pub(super) fn handle_oauth_complete(
        &mut self,
        result: Result<crate::slack::SlackAuth, anyhow::Error>,
    ) {
        match result {
            Ok(auth) => {
                info!("Slack OAuth completed successfully");
                self.toast_message = Some((
                    "Slack connected! Add app_token to slack_auth.yaml".to_string(),
                    std::time::Instant::now(),
                ));
                if let Some(ref state) = self.slack_state {
                    let st = state.clone();
                    tokio::spawn(async move {
                        st.lock().await.log(
                            crate::slack::SlackLogLevel::Info,
                            "OAuth completed successfully".to_string(),
                        );
                    });
                }
                self.slack_auth = Some(auth);
            }
            Err(e) => {
                tracing::error!("Slack OAuth failed: {}", e);
                self.toast_message = Some((
                    format!("Slack OAuth failed: {}", e),
                    std::time::Instant::now(),
                ));
                if let Some(ref state) = self.slack_state {
                    let st = state.clone();
                    let err_msg = e.to_string();
                    tokio::spawn(async move {
                        st.lock().await.log(
                            crate::slack::SlackLogLevel::Error,
                            format!("OAuth failed: {}", err_msg),
                        );
                    });
                }
            }
        }
        self.needs_redraw = true;
    }
}
