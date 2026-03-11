use crate::app::App;
use crate::slack::SlackBackgroundEvent;
use tracing::{debug, error, info, warn};

impl App {
    /// Fetch the triage session ID from the triage project.
    pub(super) async fn fetch_triage_session_id(
        client: &reqwest::Client,
        base_url: &str,
        triage_dir_str: &str,
    ) -> String {
        let sessions_url = format!("{}/session", base_url);
        let sessions_resp = client
            .get(&sessions_url)
            .header("x-opencode-directory", triage_dir_str)
            .header("Accept", "application/json")
            .send()
            .await;

        match sessions_resp {
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
                    let dir = s.get("directory").and_then(|v| v.as_str()).unwrap_or("");
                    dir == triage_dir_str
                });
                debug!(
                    "Triage: found {} sessions total, filtered to triage dir: {}",
                    items.len(),
                    triage_session.is_some()
                );
                triage_session
                    .and_then(|s| s.get("id").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string()
            }
            Err(e) => {
                warn!("Triage: failed to fetch sessions: {}", e);
                String::new()
            }
        }
    }

    /// Poll until the triage session becomes idle.
    pub(super) async fn poll_triage_idle(
        base_url: &str,
        triage_dir_str: &str,
        session_id: &str,
        label: &str,
    ) {
        let api = crate::api::ApiClient::new();
        let max_polls = 30;
        for poll in 0..max_polls {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            match api.fetch_session_status(base_url, triage_dir_str).await {
                Ok(status_map) => {
                    let is_busy = status_map
                        .get(session_id)
                        .map(|s| s == "busy")
                        .unwrap_or(false);
                    if !is_busy {
                        info!("{} idle after {}s", label, (poll + 1) * 2);
                        break;
                    }
                }
                Err(e) => {
                    warn!("Failed to poll {} status: {}", label, e);
                    break;
                }
            }
        }
    }

    /// Fetch the latest AI response from a triage session.
    pub(super) async fn fetch_triage_ai_response(
        client: &reqwest::Client,
        base_url: &str,
        triage_dir_str: &str,
        session_id: &str,
    ) -> Result<String, String> {
        match crate::slack::fetch_all_session_messages(
            client,
            base_url,
            triage_dir_str,
            session_id,
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
                Ok(ai_response)
            }
            Err(e) => Err(format!("Failed to fetch triage response: {}", e)),
        }
    }

    /// Run the triage flow: find triage session, send prompt, poll for response,
    /// and emit a `TriageResult` event.
    pub(super) async fn run_triage(
        bg_tx: tokio::sync::mpsc::UnboundedSender<crate::app::BackgroundEvent>,
        base_url: String,
        triage_dir_str: String,
        prompt: String,
        ts: String,
        channel: String,
        original_text: String,
    ) {
        let client = reqwest::Client::new();
        info!("Triage: sending prompt to detect project");

        let session_id =
            Self::fetch_triage_session_id(&client, &base_url, &triage_dir_str).await;

        if session_id.is_empty() {
            let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                SlackBackgroundEvent::TriageResult {
                    thread_ts: ts,
                    channel,
                    original_text,
                    rewritten_query: None,
                    project_path: None,
                    model: None,
                    direct_answer: None,
                    create_session: false,
                    connect_only: false,
                    error: Some(
                        "No triage session available. Please ensure the Slack triage project \
                         has at least one session."
                            .to_string(),
                    ),
                },
            ));
            return;
        }

        match crate::slack::send_user_message(
            &client, &base_url, &triage_dir_str, &session_id, &prompt,
        )
        .await
        {
            Ok(()) => {
                info!("Triage prompt sent, polling for response...");
                Self::poll_triage_idle(
                    &base_url,
                    &triage_dir_str,
                    &session_id,
                    "Triage session",
                )
                .await;

                match Self::fetch_triage_ai_response(
                    &client,
                    &base_url,
                    &triage_dir_str,
                    &session_id,
                )
                .await
                {
                    Ok(ai_response) => {
                        let (
                            project_path,
                            model,
                            rewritten_query,
                            direct_answer,
                            create_session,
                            connect_only,
                            triage_error,
                        ) = crate::slack::parse_triage_response(&ai_response);

                        let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                            SlackBackgroundEvent::TriageResult {
                                thread_ts: ts,
                                channel,
                                original_text,
                                rewritten_query,
                                project_path,
                                model,
                                direct_answer,
                                create_session,
                                connect_only,
                                error: triage_error,
                            },
                        ));
                    }
                    Err(e) => {
                        error!("{}", e);
                        let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                            SlackBackgroundEvent::TriageResult {
                                thread_ts: ts,
                                channel,
                                original_text,
                                rewritten_query: None,
                                project_path: None,
                                model: None,
                                direct_answer: None,
                                create_session: false,
                                connect_only: false,
                                error: Some(e),
                            },
                        ));
                    }
                }
            }
            Err(e) => {
                error!("Failed to send triage prompt: {}", e);
                let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                    SlackBackgroundEvent::TriageResult {
                        thread_ts: ts,
                        channel,
                        original_text,
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
    }
}
