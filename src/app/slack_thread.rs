use crate::app::App;
use tracing::{error, info};

impl App {
    /// Handle `SlackBackgroundEvent::IncomingThreadReply`.
    pub(super) fn handle_incoming_thread_reply(
        &mut self,
        text: String,
        channel: String,
        _ts: String,
        thread_ts: String,
        _user: String,
    ) {
        let state = match self.slack_state.clone() {
            Some(s) => s,
            None => return,
        };
        {
            let trimmed = text.trim().to_string();

            // ── Slash command check ──────────────────────────
            if trimmed.starts_with('@') {
                self.handle_thread_at_command(
                    &trimmed, &text, &channel, &thread_ts, &state,
                );
                return;
            }

            // ── Permission / question reply check ────────────
            if self.handle_permission_or_question_reply(&text, &channel, &thread_ts, &state) {
                return;
            }
        }

        // ── Normal thread reply ─────────────────────────
        {
            let st = state.clone();
            let base_url = crate::app::base_url().to_string();
            let projects: Vec<(String, String)> = self
                .projects
                .iter()
                .map(|p| {
                    (
                        p.path.to_string_lossy().to_string(),
                        p.path.to_string_lossy().to_string(),
                    )
                })
                .collect();
            let bg_tx = self.bg_tx.clone();

            tokio::spawn(async move {
                let mut s = st.lock().await;
                if let Some((_pidx, session_id)) = s.thread_sessions.get(&thread_ts) {
                    let client = reqwest::Client::new();
                    let project_dir = projects
                        .get(*_pidx)
                        .map(|(_, path)| path.clone())
                        .unwrap_or_default();
                    let sid = session_id.clone();

                    if let Err(e) = crate::slack::send_system_message(
                        &client,
                        &base_url,
                        &project_dir,
                        &sid,
                        &text,
                    )
                    .await
                    {
                        error!("Failed to inject thread reply as system message: {}", e);
                        s.log(
                            crate::slack::SlackLogLevel::Error,
                            format!("Thread reply injection failed: {}", e),
                        );
                    } else {
                        info!(
                            "Slack: thread reply injected as system message to session {}",
                            sid
                        );
                        match crate::slack::fetch_session_messages_with_tools(
                            &client,
                            &base_url,
                            &project_dir,
                            &sid,
                        )
                        .await
                        {
                            Ok(msgs) => {
                                s.session_msg_offset.insert(sid.clone(), msgs.len());
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Slack: failed to update msg offset after thread reply: {}",
                                    e
                                );
                            }
                        }
                        s.metrics.thread_replies += 1;
                        s.log(
                            crate::slack::SlackLogLevel::Info,
                            format!(
                                "Thread reply injected to session {}",
                                &sid[..8.min(sid.len())]
                            ),
                        );
                    }
                } else {
                    info!(
                        "Slack: unknown thread_ts={}, re-emitting as IncomingMessage for triage",
                        thread_ts
                    );
                    s.log(
                        crate::slack::SlackLogLevel::Info,
                        format!("Re-routing unknown thread {} to triage", thread_ts),
                    );
                    drop(s);
                    let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                        crate::slack::SlackBackgroundEvent::IncomingMessage {
                            text,
                            channel,
                            ts: thread_ts,
                            user: String::new(),
                        },
                    ));
                }
            });
        }
    }

    /// Handle @ commands in thread replies (e.g. @watcher, @status, etc.).
    fn handle_thread_at_command(
        &mut self,
        trimmed: &str,
        _text: &str,
        channel: &str,
        thread_ts: &str,
        state: &std::sync::Arc<tokio::sync::Mutex<crate::slack::SlackState>>,
    ) {
        let st = state.clone();
        let base_url = crate::app::base_url().to_string();
        let bot_token = self
            .slack_auth
            .as_ref()
            .map(|a| a.bot_token.clone())
            .unwrap_or_default();
        let projects: Vec<(usize, String)> = self
            .projects
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.path.to_string_lossy().to_string()))
            .collect();

        let mut watcher_inserted = false;
        let mut watcher_removed = false;
        let mut watcher_trigger_sid: Option<String> = None;

        if trimmed == "@watcher" || trimmed.starts_with("@watcher ") {
            let watcher_args = trimmed.strip_prefix("@watcher").unwrap_or("").trim();

            if watcher_args == "stop" || watcher_args == "off" || watcher_args == "remove" {
                if let Ok(s) = state.try_lock() {
                    if let Some((_pidx, session_id)) = s.thread_sessions.get(thread_ts) {
                        let sid = session_id.clone();
                        drop(s);
                        if self.session_watchers.remove(&sid).is_some() {
                            self.watcher_idle_since.remove(&sid);
                            if let Some(handle) = self.watcher_pending.remove(&sid) {
                                handle.abort();
                            }
                            watcher_removed = true;
                            tracing::info!(
                                "Slack @watcher stop: removed watcher for session {}",
                                &sid[..8.min(sid.len())]
                            );
                        }
                    }
                }
            } else {
                if let Ok(s) = state.try_lock() {
                    if let Some((pidx, session_id)) = s.thread_sessions.get(thread_ts) {
                        let sid = session_id.clone();
                        let pidx_val = *pidx;
                        drop(s);
                        let config = crate::app::WatcherConfig {
                            session_id: sid.clone(),
                            project_idx: pidx_val,
                            idle_timeout_secs: 15,
                            continuation_message: "Continue if you have next steps, or stop and ask for clarification if you are unsure how to proceed.".to_string(),
                            include_original: false,
                            original_message: None,
                            hang_message: "You appear to be stuck or hanging. Please abort your current approach and try a different strategy.".to_string(),
                            hang_timeout_secs: 180,
                        };
                        self.session_watchers.insert(sid.clone(), config);
                        watcher_inserted = true;
                        watcher_trigger_sid = Some(sid.clone());
                        tracing::info!(
                            "Slack @watcher: enabled watcher for session {}",
                            &sid[..8.min(sid.len())]
                        );
                    }
                }
            }
        }

        let trimmed_owned = trimmed.to_string();
        let channel_owned = channel.to_string();
        let thread_ts_owned = thread_ts.to_string();
        tokio::spawn(async move {
            let s = st.lock().await;
            if let Some((pidx, session_id)) = s.thread_sessions.get(&thread_ts_owned) {
                let sid = session_id.clone();
                let pidx_val = *pidx;
                let project_dir = projects
                    .iter()
                    .find(|(i, _)| *i == pidx_val)
                    .map(|(_, p)| p.clone())
                    .unwrap_or_default();
                drop(s);

                crate::slack::handle_thread_slash_command(
                    &trimmed_owned,
                    &channel_owned,
                    &thread_ts_owned,
                    &sid,
                    pidx_val,
                    &project_dir,
                    &bot_token,
                    &base_url,
                    &st,
                    watcher_inserted,
                    watcher_removed,
                )
                .await;
            } else {
                drop(s);
                let client = reqwest::Client::new();
                let _ = crate::slack::post_message(
                    &client,
                    &bot_token,
                    &channel_owned,
                    ":warning: No session mapped to this thread.",
                    Some(&thread_ts_owned),
                )
                .await;
            }
        });

        // Trigger watcher immediately if session already idle.
        if let Some(sid) = watcher_trigger_sid {
            if !self.active_sessions.contains(&sid) {
                let has_active_children = self
                    .session_children
                    .get(&sid)
                    .map(|children| children.iter().any(|cid| self.active_sessions.contains(cid)))
                    .unwrap_or(false);
                self.try_trigger_watcher(&sid, has_active_children);
            }
        }
    }

}
