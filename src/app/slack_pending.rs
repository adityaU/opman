use crate::app::App;
use tracing::{error, info};

impl App {
    /// Drain queued Slack messages for a project once a session becomes available.
    pub(super) fn drain_pending_slack_messages(
        &mut self,
        project_idx: usize,
        session_id: &str,
    ) {
        // Extract messages for this project.
        let (to_send, remaining): (Vec<_>, Vec<_>) = self
            .pending_slack_messages
            .drain(..)
            .partition(|m| m.project_idx == project_idx);

        self.pending_slack_messages = remaining;

        if to_send.is_empty() {
            return;
        }

        let project = match self.projects.get(project_idx) {
            Some(p) => p,
            None => return,
        };

        let bot_token = self
            .slack_auth
            .as_ref()
            .map(|a| a.bot_token.clone())
            .unwrap_or_default();
        let base_url = crate::app::base_url().to_string();
        let project_dir = project.path.to_string_lossy().to_string();
        let pname = project.name.clone();
        let sname = project
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| s.title.clone())
            .unwrap_or_else(|| session_id[..8.min(session_id.len())].to_string());
        let sid = session_id.to_string();
        let slack_state = self.slack_state.clone();
        let buffer_secs = self.config.settings.slack.relay_buffer_secs;
        let pidx = project_idx;

        for msg in to_send {
            let sid = sid.clone();
            let bot_token = bot_token.clone();
            let base_url = base_url.clone();
            let project_dir = project_dir.clone();
            let pname = pname.clone();
            let sname = sname.clone();
            let slack_state = slack_state.clone();

            let text = msg.rewritten_query.unwrap_or(msg.original_text.clone());

            info!(
                "Slack: draining queued message to session {} in project {}",
                &sid[..8.min(sid.len())],
                pname
            );

            tokio::spawn(async move {
                let client = reqwest::Client::new();

                // Record the thread->session mapping.
                if let Some(ref st) = slack_state {
                    let mut s = st.lock().await;
                    s.thread_sessions
                        .insert(msg.thread_ts.clone(), (pidx, sid.clone()));
                    s.session_threads
                        .insert(sid.clone(), (msg.channel.clone(), msg.thread_ts.clone()));
                    s.metrics.messages_routed += 1;
                    s.metrics.last_routed_at = Some(std::time::Instant::now());
                    s.log(
                        crate::slack::SlackLogLevel::Info,
                        format!(
                            "Routed queued message to project \"{}\" session {}",
                            pname,
                            &sid[..8.min(sid.len())]
                        ),
                    );
                }

                // Record current message offset.
                if let Some(ref st) = slack_state {
                    match crate::slack::fetch_session_messages_with_tools(
                        &client,
                        &base_url,
                        &project_dir,
                        &sid,
                    )
                    .await
                    {
                        Ok(msgs) => {
                            let mut s = st.lock().await;
                            s.session_msg_offset.insert(sid.clone(), msgs.len());
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Slack: failed to fetch msg offset for queued message: {}",
                                e
                            );
                        }
                    }
                }

                // Send the user message.
                match crate::slack::send_user_message(
                    &client, &base_url, &project_dir, &sid, &text,
                )
                .await
                {
                    Ok(()) => {
                        info!("Slack: queued message sent to session {}", sid);
                        let ack =
                            format!("relayed to project: {}, session: {}", pname, sname);
                        let _ = crate::slack::post_message(
                            &client,
                            &bot_token,
                            &msg.channel,
                            &ack,
                            Some(&msg.thread_ts),
                        )
                        .await;
                    }
                    Err(e) => {
                        error!("Failed to send queued message to session: {}", e);
                        let err_msg = format!("opman: Failed to send message: {}", e);
                        let _ = crate::slack::post_message(
                            &client,
                            &bot_token,
                            &msg.channel,
                            &err_msg,
                            Some(&msg.thread_ts),
                        )
                        .await;
                    }
                }

                // Spawn relay watcher if not already running.
                if let Some(ref st) = slack_state {
                    let already_watching = {
                        let s = st.lock().await;
                        s.relay_abort_handles.contains_key(&sid)
                    };
                    if !already_watching {
                        let handle = crate::slack::spawn_session_relay_watcher(
                            sid.clone(),
                            project_dir.clone(),
                            msg.channel.clone(),
                            msg.thread_ts.clone(),
                            bot_token.clone(),
                            base_url.clone(),
                            buffer_secs,
                            st.clone(),
                        );
                        let mut s = st.lock().await;
                        s.relay_abort_handles
                            .insert(sid.clone(), handle.abort_handle());
                        let map = crate::slack::SlackSessionMap {
                            session_threads: s.session_threads.clone(),
                            thread_sessions: s.thread_sessions.clone(),
                            msg_offsets: s.session_msg_offset.clone(),
                        };
                        if let Err(e) = map.save() {
                            tracing::warn!(
                                "Slack: failed to persist session map: {}",
                                e
                            );
                        }
                    }
                }
            });
        }
    }
}
