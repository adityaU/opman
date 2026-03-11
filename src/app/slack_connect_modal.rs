use crate::app::App;
use crate::slack::SlackBackgroundEvent;
use tracing::{error, info, warn};

impl App {
    /// Handle the connect modal submission (step 2: session + message).
    pub(super) fn handle_connect_modal_submission(
        &mut self,
        values: &serde_json::Value,
        private_metadata: &str,
    ) {
        let selected_session_id = values
            .pointer(
                "/session_select_block/session_select_action/selected_option/value",
            )
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let message = values
            .pointer("/message_input_block/message_input_action/value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let channel = private_metadata.to_string();

        if selected_session_id.is_empty() || channel.is_empty() {
            warn!("ViewSubmission (connect_modal): missing session_id or channel");
            return;
        }

        let triage_text = if message.is_empty() {
            format!("connect to session {}", selected_session_id)
        } else {
            format!(
                "connect to session {} and send: {}",
                selected_session_id, message
            )
        };

        let projects: Vec<(String, String)> = self
            .projects
            .iter()
            .filter(|p| p.name != "slack-triage")
            .map(|p| (p.name.clone(), p.path.to_string_lossy().to_string()))
            .collect();
        let sessions: Vec<crate::slack::SessionMeta> = self.collect_session_meta();

        let bot_token = self
            .slack_auth
            .as_ref()
            .map(|a| a.bot_token.clone())
            .unwrap_or_default();
        let bg_tx = self.bg_tx.clone();
        let base_url = crate::app::base_url().to_string();

        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let full_cmd = if message.is_empty() {
                format!(
                    "/opman-connect {}",
                    &selected_session_id[..8.min(selected_session_id.len())]
                )
            } else {
                format!(
                    "/opman-connect {} {}",
                    &selected_session_id[..8.min(selected_session_id.len())],
                    message
                )
            };
            let ts = match crate::slack::post_message(
                &client,
                &bot_token,
                &channel,
                &format!(":hourglass_flowing_sand: Processing `{}`...", full_cmd),
                None,
            )
            .await
            {
                Ok(ts) => ts,
                Err(e) => {
                    error!("Connect modal: failed to post processing msg: {}", e);
                    return;
                }
            };

            let prompt =
                crate::slack::build_triage_prompt(&projects, &sessions, &triage_text);

            if let Ok(triage_dir) = crate::slack::triage_project_dir() {
                let triage_dir_str = triage_dir.to_string_lossy().to_string();

                let sessions_url = format!("{}/session", base_url);
                let sessions_resp = client
                    .get(&sessions_url)
                    .header("x-opencode-directory", &triage_dir_str)
                    .header("Accept", "application/json")
                    .send()
                    .await;

                let session_id = match sessions_resp {
                    Ok(resp) => {
                        let body: serde_json::Value =
                            resp.json().await.unwrap_or_default();
                        let items: Vec<&serde_json::Value> =
                            if let Some(arr) = body.as_array() {
                                arr.iter().collect()
                            } else if let Some(obj) = body.as_object() {
                                obj.values().collect()
                            } else {
                                vec![]
                            };
                        let triage_session = items.iter().find(|s| {
                            let dir = s
                                .get("directory")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            dir == triage_dir_str
                        });
                        triage_session
                            .and_then(|s| s.get("id").and_then(|v| v.as_str()))
                            .unwrap_or("")
                            .to_string()
                    }
                    Err(e) => {
                        warn!(
                            "Connect modal triage: failed to fetch sessions: {}",
                            e
                        );
                        String::new()
                    }
                };

                if session_id.is_empty() {
                    let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                        SlackBackgroundEvent::TriageResult {
                            thread_ts: ts,
                            channel,
                            original_text: triage_text,
                            rewritten_query: None,
                            project_path: None,
                            model: None,
                            direct_answer: None,
                            create_session: false,
                            connect_only: true,
                            error: Some("No triage session available.".to_string()),
                        },
                    ));
                    return;
                }

                match crate::slack::send_user_message(
                    &client,
                    &base_url,
                    &triage_dir_str,
                    &session_id,
                    &prompt,
                )
                .await
                {
                    Ok(()) => {
                        info!("Connect modal triage prompt sent, polling...");

                        let api = crate::api::ApiClient::new();
                        let max_polls = 30;
                        for poll in 0..max_polls {
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            match api
                                .fetch_session_status(&base_url, &triage_dir_str)
                                .await
                            {
                                Ok(status_map) => {
                                    let is_busy = status_map
                                        .get(&session_id)
                                        .map(|s| s == "busy")
                                        .unwrap_or(false);
                                    if !is_busy {
                                        info!(
                                            "Connect modal triage idle after {}s",
                                            (poll + 1) * 2
                                        );
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Connect modal triage poll err: {}", e);
                                    break;
                                }
                            }
                        }

                        match crate::slack::fetch_all_session_messages(
                            &client,
                            &base_url,
                            &triage_dir_str,
                            &session_id,
                        )
                        .await
                        {
                            Ok(messages) => {
                                let ai_response = messages
                                    .iter()
                                    .rev()
                                    .find(|(role, _)| role == "assistant")
                                    .map(|(_, text)| text.clone())
                                    .unwrap_or_default();

                                let (
                                    project_path,
                                    model,
                                    rewritten_query,
                                    direct_answer,
                                    create_session,
                                    _connect_only,
                                    error_val,
                                ) = crate::slack::parse_triage_response(&ai_response);

                                let final_query = if !message.is_empty() {
                                    Some(message)
                                } else {
                                    rewritten_query
                                };

                                let _ = bg_tx.send(
                                    crate::app::BackgroundEvent::SlackEvent(
                                        SlackBackgroundEvent::TriageResult {
                                            thread_ts: ts,
                                            channel,
                                            original_text: triage_text,
                                            rewritten_query: final_query,
                                            project_path,
                                            model,
                                            direct_answer,
                                            create_session,
                                            connect_only: true,
                                            error: error_val,
                                        },
                                    ),
                                );
                            }
                            Err(e) => {
                                error!("Connect modal triage fetch err: {}", e);
                                let _ = bg_tx.send(
                                    crate::app::BackgroundEvent::SlackEvent(
                                        SlackBackgroundEvent::TriageResult {
                                            thread_ts: ts,
                                            channel,
                                            original_text: triage_text,
                                            rewritten_query: None,
                                            project_path: None,
                                            model: None,
                                            direct_answer: None,
                                            create_session: false,
                                            connect_only: true,
                                            error: Some(format!(
                                                "Failed to fetch triage response: {}",
                                                e
                                            )),
                                        },
                                    ),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        error!("Connect modal triage send err: {}", e);
                        let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                            SlackBackgroundEvent::TriageResult {
                                thread_ts: ts,
                                channel,
                                original_text: triage_text,
                                rewritten_query: None,
                                project_path: None,
                                model: None,
                                direct_answer: None,
                                create_session: false,
                                connect_only: true,
                                error: Some(format!("Triage failed: {}", e)),
                            },
                        ));
                    }
                }
            } else {
                error!("Connect modal: triage_project_dir() failed");
                let _ = crate::slack::post_message(
                    &client,
                    &bot_token,
                    &channel,
                    "opman: Internal error — could not locate triage project.",
                    Some(&ts),
                )
                .await;
            }
        });
    }
}
