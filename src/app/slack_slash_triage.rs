use crate::app::App;
use crate::slack::SlackBackgroundEvent;
use tracing::{error, info, warn};

impl App {
    /// Run the triage flow for a slash command that needs it.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn run_slash_triage(
        bg_tx: tokio::sync::mpsc::UnboundedSender<crate::app::BackgroundEvent>,
        base_url: String,
        bot_token: String,
        projects: Vec<(String, String)>,
        sessions: Vec<crate::slack::SessionMeta>,
        triage_text: String,
        txt: String,
        ts: String,
        ch: String,
        force_connect: bool,
        force_route: bool,
    ) {
        let prompt = crate::slack::build_triage_prompt(&projects, &sessions, &triage_text);

        if let Ok(triage_dir) = crate::slack::triage_project_dir() {
            let triage_dir_str = triage_dir.to_string_lossy().to_string();
            let client = reqwest::Client::new();

            info!("Slash command triage: sending prompt");

            let sessions_url = format!("{}/session", base_url);
            let sessions_resp = client
                .get(&sessions_url)
                .header("x-opencode-directory", &triage_dir_str)
                .header("Accept", "application/json")
                .send()
                .await;

            let session_id = match sessions_resp {
                Ok(resp) => {
                    let body: serde_json::Value = resp.json().await.unwrap_or_default();
                    let items: Vec<&serde_json::Value> = if let Some(arr) = body.as_array() {
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
                    warn!("Slash triage: failed to fetch sessions: {}", e);
                    String::new()
                }
            };

            if session_id.is_empty() {
                let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                    SlackBackgroundEvent::TriageResult {
                        thread_ts: ts,
                        channel: ch,
                        original_text: triage_text,
                        rewritten_query: None,
                        project_path: None,
                        model: None,
                        direct_answer: None,
                        create_session: false,
                        connect_only: false,
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
                    info!("Slash triage prompt sent, polling...");

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
                                    info!("Slash triage idle after {}s", (poll + 1) * 2);
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Slash triage poll err: {}", e);
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
                                mut connect_only,
                                error_val,
                            ) = crate::slack::parse_triage_response(&ai_response);

                            if force_connect {
                                connect_only = true;
                            }

                            let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                                SlackBackgroundEvent::TriageResult {
                                    thread_ts: ts,
                                    channel: ch,
                                    original_text: triage_text,
                                    rewritten_query: if force_route {
                                        rewritten_query.or(Some(txt))
                                    } else {
                                        rewritten_query
                                    },
                                    project_path,
                                    model,
                                    direct_answer,
                                    create_session,
                                    connect_only,
                                    error: error_val,
                                },
                            ));
                        }
                        Err(e) => {
                            error!("Slash triage fetch err: {}", e);
                            let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                                SlackBackgroundEvent::TriageResult {
                                    thread_ts: ts,
                                    channel: ch,
                                    original_text: triage_text,
                                    rewritten_query: None,
                                    project_path: None,
                                    model: None,
                                    direct_answer: None,
                                    create_session: false,
                                    connect_only: false,
                                    error: Some(format!(
                                        "Failed to fetch triage response: {}",
                                        e
                                    )),
                                },
                            ));
                        }
                    }
                }
                Err(e) => {
                    error!("Slash triage send err: {}", e);
                    let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                        SlackBackgroundEvent::TriageResult {
                            thread_ts: ts,
                            channel: ch,
                            original_text: triage_text,
                            rewritten_query: None,
                            project_path: None,
                            model: None,
                            direct_answer: None,
                            create_session: false,
                            connect_only: false,
                            error: Some(format!("Triage failed: {}", e)),
                        },
                    ));
                }
            }
        } else {
            error!("Slash command: triage_project_dir() failed");
            let client = reqwest::Client::new();
            let _ = crate::slack::post_message(
                &client,
                &bot_token,
                &ch,
                "opman: Internal error — could not locate triage project.",
                Some(&ts),
            )
            .await;
        }
    }
}
