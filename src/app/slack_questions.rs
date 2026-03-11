use crate::app::App;

impl App {
    /// Handle permission and question replies in thread messages.
    /// Returns `true` if the reply was handled as a permission/question response.
    pub(super) fn handle_permission_or_question_reply(
        &mut self,
        text: &str,
        channel: &str,
        thread_ts: &str,
        state: &std::sync::Arc<tokio::sync::Mutex<crate::slack::SlackState>>,
    ) -> bool {
        let trimmed_lower = text.trim().to_lowercase();
        let mut handled = false;

        if let Ok(mut guard) = state.try_lock() {
            if let Some((req_id, _sid, pidx, _msg_ts, _perm_req)) =
                guard.pending_permissions.get(thread_ts).cloned()
            {
                let reply = match trimmed_lower.as_str() {
                    "once" | "yes" | "allow" | "1" => Some("once"),
                    "always" | "all" | "2" => Some("always"),
                    "reject" | "deny" | "no" | "3" => Some("reject"),
                    _ => None,
                };

                if let Some(reply_str) = reply {
                    guard.pending_permissions.remove(thread_ts);
                    drop(guard);

                    let base_url = crate::app::base_url().to_string();
                    let project_dir = self
                        .projects
                        .get(pidx)
                        .map(|p| p.path.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let bot_token = self
                        .slack_auth
                        .as_ref()
                        .map(|a| a.bot_token.clone())
                        .unwrap_or_default();
                    let ch = channel.to_string();
                    let tts = thread_ts.to_string();
                    let rid = req_id.clone();
                    let reply_owned = reply_str.to_string();

                    tokio::spawn(async move {
                        let api = crate::api::ApiClient::new();
                        match api
                            .reply_permission(&base_url, &project_dir, &rid, &reply_owned)
                            .await
                        {
                            Ok(()) => {
                                let client = reqwest::Client::new();
                                let _ = crate::slack::post_message(
                                    &client,
                                    &bot_token,
                                    &ch,
                                    &format!(
                                        ":white_check_mark: Permission reply sent: `{}`",
                                        reply_owned
                                    ),
                                    Some(&tts),
                                )
                                .await;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Slack: failed to reply to permission {}: {}",
                                    rid,
                                    e
                                );
                                let client = reqwest::Client::new();
                                let _ = crate::slack::post_message(
                                    &client,
                                    &bot_token,
                                    &ch,
                                    &format!(":x: Failed to send permission reply: {}", e),
                                    Some(&tts),
                                )
                                .await;
                            }
                        }
                    });

                    return true;
                }
            }
        }

        if !handled {
            handled = self.handle_question_reply(text, channel, thread_ts, state);
        }

        handled
    }

    /// Handle question replies from thread messages.
    /// Returns `true` if the reply was handled as a question response.
    pub(super) fn handle_question_reply(
        &mut self,
        text: &str,
        channel: &str,
        thread_ts: &str,
        state: &std::sync::Arc<tokio::sync::Mutex<crate::slack::SlackState>>,
    ) -> bool {
        let trimmed_lower = text.trim().to_lowercase();

        if let Ok(mut guard) = state.try_lock() {
            if let Some((req_id, _sid, pidx, _msg_ts, ref question_req)) =
                guard.pending_questions.get(thread_ts).cloned()
            {
                let is_reject = trimmed_lower == "reject"
                    || trimmed_lower == "dismiss"
                    || trimmed_lower == "skip";

                if is_reject {
                    guard.pending_questions.remove(thread_ts);
                    drop(guard);

                    let base_url = crate::app::base_url().to_string();
                    let project_dir = self
                        .projects
                        .get(pidx)
                        .map(|p| p.path.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let bot_token = self
                        .slack_auth
                        .as_ref()
                        .map(|a| a.bot_token.clone())
                        .unwrap_or_default();
                    let ch = channel.to_string();
                    let tts = thread_ts.to_string();
                    let rid = req_id.clone();

                    tokio::spawn(async move {
                        let api = crate::api::ApiClient::new();
                        match api.reject_question(&base_url, &project_dir, &rid).await {
                            Ok(()) => {
                                let client = reqwest::Client::new();
                                let _ = crate::slack::post_message(
                                    &client,
                                    &bot_token,
                                    &ch,
                                    ":no_entry_sign: Question dismissed.",
                                    Some(&tts),
                                )
                                .await;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Slack: failed to reject question {}: {}",
                                    rid,
                                    e
                                );
                            }
                        }
                    });

                    return true;
                }

                // Parse option numbers or use as custom text.
                let mut answers: Vec<Vec<String>> = Vec::new();
                let reply_text = text.trim().to_string();

                for q in &question_req.questions {
                    let parts: Vec<&str> = reply_text.split(',').map(|s| s.trim()).collect();
                    let mut selected_labels: Vec<String> = Vec::new();
                    let mut all_numeric = true;

                    for part in &parts {
                        if let Ok(n) = part.parse::<usize>() {
                            if n >= 1 && n <= q.options.len() {
                                selected_labels.push(q.options[n - 1].label.clone());
                            } else {
                                all_numeric = false;
                                break;
                            }
                        } else {
                            all_numeric = false;
                            break;
                        }
                    }

                    if all_numeric && !selected_labels.is_empty() {
                        answers.push(selected_labels);
                    } else {
                        answers.push(vec![reply_text.clone()]);
                    }
                }

                guard.pending_questions.remove(thread_ts);
                drop(guard);

                let base_url = crate::app::base_url().to_string();
                let project_dir = self
                    .projects
                    .get(pidx)
                    .map(|p| p.path.to_string_lossy().to_string())
                    .unwrap_or_default();
                let bot_token = self
                    .slack_auth
                    .as_ref()
                    .map(|a| a.bot_token.clone())
                    .unwrap_or_default();
                let ch = channel.to_string();
                let tts = thread_ts.to_string();
                let rid = req_id.clone();

                tokio::spawn(async move {
                    let api = crate::api::ApiClient::new();
                    match api
                        .reply_question(&base_url, &project_dir, &rid, &answers)
                        .await
                    {
                        Ok(()) => {
                            let answer_display = answers
                                .iter()
                                .map(|a| a.join(", "))
                                .collect::<Vec<_>>()
                                .join(" | ");
                            let client = reqwest::Client::new();
                            let _ = crate::slack::post_message(
                                &client,
                                &bot_token,
                                &ch,
                                &format!(":white_check_mark: Answer sent: {}", answer_display),
                                Some(&tts),
                            )
                            .await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Slack: failed to reply to question {}: {}",
                                rid,
                                e
                            );
                            let client = reqwest::Client::new();
                            let _ = crate::slack::post_message(
                                &client,
                                &bot_token,
                                &ch,
                                &format!(":x: Failed to send question answer: {}", e),
                                Some(&tts),
                            )
                            .await;
                        }
                    }
                });

                return true;
            }
        }

        false
    }
}
