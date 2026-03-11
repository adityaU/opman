use crate::app::App;
use tracing::info;

impl App {
    /// Handle `SlackBackgroundEvent::BlockAction`.
    pub(super) fn handle_block_action(
        &mut self,
        action_id: String,
        channel: String,
        message_ts: String,
        thread_ts: Option<String>,
        _user: String,
    ) {
        info!(
            action_id = %action_id,
            channel = %channel,
            message_ts = %message_ts,
            thread_ts = ?thread_ts,
            "Slack block action received"
        );

        if let (Some(ref ss), Some(ref auth)) =
            (self.slack_state.clone(), self.slack_auth.clone())
        {
            let bot_token = auth.bot_token.clone();
            let base_url = crate::app::base_url().to_string();
            let ss = ss.clone();
            let project_paths: Vec<String> = self
                .projects
                .iter()
                .map(|p| p.path.to_string_lossy().to_string())
                .collect();

            tokio::spawn(async move {
                let thread_key = thread_ts.clone().unwrap_or_default();

                if action_id.starts_with("perm_") {
                    Self::handle_permission_block_action(
                        &action_id,
                        &channel,
                        &message_ts,
                        &thread_ts,
                        &thread_key,
                        &bot_token,
                        &base_url,
                        &ss,
                        &project_paths,
                    )
                    .await;
                } else if action_id.starts_with("q_reject:") {
                    Self::handle_question_reject_action(
                        &channel,
                        &message_ts,
                        &thread_key,
                        &bot_token,
                        &base_url,
                        &ss,
                        &project_paths,
                    )
                    .await;
                } else if action_id.starts_with("q_") {
                    Self::handle_question_option_action(
                        &action_id,
                        &channel,
                        &message_ts,
                        &thread_ts,
                        &thread_key,
                        &bot_token,
                        &base_url,
                        &ss,
                        &project_paths,
                    )
                    .await;
                } else {
                    tracing::debug!("Slack: ignoring unknown action_id: {}", action_id);
                }
            });
        }
    }

    /// Handle permission button actions (perm_once, perm_always, perm_reject).
    async fn handle_permission_block_action(
        action_id: &str,
        channel: &str,
        message_ts: &str,
        thread_ts: &Option<String>,
        thread_key: &str,
        bot_token: &str,
        base_url: &str,
        ss: &std::sync::Arc<tokio::sync::Mutex<crate::slack::SlackState>>,
        project_paths: &[String],
    ) {
        let (reply_str, _req_id_from_action) =
            if let Some(rid) = action_id.strip_prefix("perm_once:") {
                ("once", rid.to_string())
            } else if let Some(rid) = action_id.strip_prefix("perm_always:") {
                ("always", rid.to_string())
            } else if let Some(rid) = action_id.strip_prefix("perm_reject:") {
                ("reject", rid.to_string())
            } else {
                tracing::warn!("Slack: unrecognized permission action_id: {}", action_id);
                return;
            };

        let pending = {
            let mut guard = ss.lock().await;
            guard.pending_permissions.remove(thread_key)
        };

        if let Some((req_id, _sid, pidx, _orig_msg_ts, perm_req)) = pending {
            let project_dir = project_paths.get(pidx).cloned().unwrap_or_default();

            let api = crate::api::ApiClient::new();
            match api
                .reply_permission(base_url, &project_dir, &req_id, reply_str)
                .await
            {
                Ok(()) => {
                    let (confirmed_text, confirmed_blocks) =
                        crate::slack::render_permission_confirmed_blocks(&perm_req, reply_str);
                    let client = reqwest::Client::new();
                    let _ = crate::slack::update_message_blocks(
                        &client,
                        bot_token,
                        channel,
                        message_ts,
                        &confirmed_text,
                        &confirmed_blocks,
                        None,
                    )
                    .await;
                }
                Err(e) => {
                    tracing::warn!(
                        "Slack: failed to reply to permission via button: {}",
                        e
                    );
                    let client = reqwest::Client::new();
                    let tts = thread_ts.as_deref().unwrap_or(channel);
                    let _ = crate::slack::post_message(
                        &client,
                        bot_token,
                        channel,
                        &format!(":x: Failed to send permission reply: {}", e),
                        Some(tts),
                    )
                    .await;
                }
            }
        } else {
            tracing::warn!(
                "Slack: no pending permission found for thread {:?}",
                thread_ts
            );
        }
    }

    /// Handle question dismiss button action.
    async fn handle_question_reject_action(
        channel: &str,
        message_ts: &str,
        thread_key: &str,
        bot_token: &str,
        base_url: &str,
        ss: &std::sync::Arc<tokio::sync::Mutex<crate::slack::SlackState>>,
        project_paths: &[String],
    ) {
        let pending = {
            let mut guard = ss.lock().await;
            guard.pending_questions.remove(thread_key)
        };

        if let Some((req_id, _sid, pidx, _orig_msg_ts, _q_req)) = pending {
            let project_dir = project_paths.get(pidx).cloned().unwrap_or_default();

            let api = crate::api::ApiClient::new();
            match api.reject_question(base_url, &project_dir, &req_id).await {
                Ok(()) => {
                    let (dismissed_text, dismissed_blocks) =
                        crate::slack::render_question_dismissed_blocks();
                    let client = reqwest::Client::new();
                    let _ = crate::slack::update_message_blocks(
                        &client,
                        bot_token,
                        channel,
                        message_ts,
                        &dismissed_text,
                        &dismissed_blocks,
                        None,
                    )
                    .await;
                }
                Err(e) => {
                    tracing::warn!("Slack: failed to reject question via button: {}", e);
                }
            }
        }
    }

    /// Handle question option button action.
    async fn handle_question_option_action(
        action_id: &str,
        channel: &str,
        message_ts: &str,
        thread_ts: &Option<String>,
        thread_key: &str,
        bot_token: &str,
        base_url: &str,
        ss: &std::sync::Arc<tokio::sync::Mutex<crate::slack::SlackState>>,
        project_paths: &[String],
    ) {
        let rest = &action_id[2..]; // strip "q_"
        if let Some(colon_pos) = rest.rfind(':') {
            let before_colon = &rest[..colon_pos];
            let option_idx_str = &rest[colon_pos + 1..];

            if let Some(last_underscore) = before_colon.rfind('_') {
                let qi_str = &before_colon[last_underscore + 1..];

                let qi: usize = qi_str.parse().unwrap_or(0);
                let oi: usize = option_idx_str.parse().unwrap_or(0);

                let pending = {
                    let mut guard = ss.lock().await;
                    guard.pending_questions.remove(thread_key)
                };

                if let Some((req_id, _sid, pidx, _orig_msg_ts, q_req)) = pending {
                    let mut answers: Vec<Vec<String>> = Vec::new();
                    for (i, q) in q_req.questions.iter().enumerate() {
                        if i == qi {
                            if let Some(opt) = q.options.get(oi) {
                                answers.push(vec![opt.label.clone()]);
                            } else {
                                answers.push(vec![format!("option_{}", oi)]);
                            }
                        } else {
                            if let Some(first) = q.options.first() {
                                answers.push(vec![first.label.clone()]);
                            } else {
                                answers.push(vec![]);
                            }
                        }
                    }

                    let project_dir = project_paths.get(pidx).cloned().unwrap_or_default();

                    let api = crate::api::ApiClient::new();
                    match api
                        .reply_question(base_url, &project_dir, &req_id, &answers)
                        .await
                    {
                        Ok(()) => {
                            let answer_display = answers
                                .iter()
                                .map(|a| a.join(", "))
                                .collect::<Vec<_>>()
                                .join(" | ");
                            let (confirmed_text, confirmed_blocks) =
                                crate::slack::render_question_confirmed_blocks(&answer_display);
                            let client = reqwest::Client::new();
                            let _ = crate::slack::update_message_blocks(
                                &client,
                                bot_token,
                                channel,
                                message_ts,
                                &confirmed_text,
                                &confirmed_blocks,
                                None,
                            )
                            .await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Slack: failed to reply to question via button: {}",
                                e
                            );
                            let client = reqwest::Client::new();
                            let tts = thread_ts.as_deref().unwrap_or(channel);
                            let _ = crate::slack::post_message(
                                &client,
                                bot_token,
                                channel,
                                &format!(":x: Failed to send question answer: {}", e),
                                Some(tts),
                            )
                            .await;
                        }
                    }
                }
            }
        }
    }
}
