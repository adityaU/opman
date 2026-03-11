use anyhow::Result;

use crate::app::App;
use crate::command_palette::CommandAction;

/// Handle Slack-related command actions.
pub(super) fn handle_slack_action(app: &mut App, action: CommandAction) -> Result<()> {
    match action {
        CommandAction::SlackConnect => {
            // If Slack auth exists, show status. Otherwise, prompt for OAuth.
            match crate::slack::SlackAuth::load() {
                Ok(Some(auth)) => {
                    if auth.app_token.is_empty() {
                        app.toast_message = Some((
                            "Slack: bot_token present but app_token missing. Add it to slack_auth.yaml.".to_string(),
                            std::time::Instant::now(),
                        ));
                    } else if app.slack_state.is_some() {
                        // Already connected — show status.
                        app.toast_message = Some((
                            "Slack: already connected via Socket Mode.".to_string(),
                            std::time::Instant::now(),
                        ));
                    } else {
                        // Have full credentials but not started — start now.
                        let slack_state = std::sync::Arc::new(tokio::sync::Mutex::new(
                            crate::slack::SlackState::new(),
                        ));
                        app.slack_state = Some(slack_state.clone());
                        app.slack_auth = Some(auth.clone());

                        let bg_tx = app.bg_tx.clone();
                        let auth_clone = auth.clone();
                        tokio::spawn(async move {
                            let (slack_event_tx, mut slack_event_rx) =
                                tokio::sync::mpsc::unbounded_channel();
                            tokio::spawn(crate::slack::spawn_socket_mode(
                                auth_clone,
                                slack_event_tx,
                            ));
                            while let Some(event) = slack_event_rx.recv().await {
                                let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(event));
                            }
                        });

                        let batch_secs = app.config.settings.slack.response_batch_secs;
                        let state_batch = slack_state;
                        let auth_batch = auth;
                        tokio::spawn(async move {
                            let (event_tx, _) = tokio::sync::mpsc::unbounded_channel();
                            crate::slack::spawn_response_batcher(
                                auth_batch,
                                state_batch,
                                batch_secs,
                                event_tx,
                            )
                            .await;
                        });

                        app.toast_message = Some((
                            "Slack: connecting via Socket Mode...".to_string(),
                            std::time::Instant::now(),
                        ));
                    }
                }
                Ok(None) => {
                    app.toast_message = Some((
                        "No slack_auth.yaml found. Create it with your tokens.".to_string(),
                        std::time::Instant::now(),
                    ));
                }
                Err(e) => {
                    app.toast_message = Some((
                        format!("Failed to load Slack auth: {}", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
        CommandAction::SlackOAuth => {
            // Trigger OAuth flow if we have credentials for client_id/client_secret.
            match crate::slack::SlackAuth::load() {
                Ok(Some(auth)) if !auth.client_id.is_empty() && !auth.client_secret.is_empty() => {
                    let client_id = auth.client_id.clone();
                    let client_secret = auth.client_secret.clone();
                    let bg_tx = app.bg_tx.clone();
                    app.toast_message = Some((
                        "Slack: starting OAuth flow, check your browser...".to_string(),
                        std::time::Instant::now(),
                    ));
                    tokio::spawn(async move {
                        let result = crate::slack::run_oauth_flow(&client_id, &client_secret).await;
                        let _ = bg_tx.send(crate::app::BackgroundEvent::SlackEvent(
                            crate::slack::SlackBackgroundEvent::OAuthComplete(result),
                        ));
                    });
                }
                Ok(Some(_)) => {
                    app.toast_message = Some((
                        "Slack: client_id/client_secret missing in slack_auth.yaml".to_string(),
                        std::time::Instant::now(),
                    ));
                }
                Ok(None) => {
                    app.toast_message = Some((
                        "No slack_auth.yaml found. Create it with client_id and client_secret first.".to_string(),
                        std::time::Instant::now(),
                    ));
                }
                Err(e) => {
                    app.toast_message = Some((
                        format!("Slack auth error: {}", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
        CommandAction::SlackDisconnect => {
            if app.slack_state.is_some() {
                app.slack_state = None;
                app.slack_auth = None;
                app.toast_message = Some((
                    "Slack: disconnected.".to_string(),
                    std::time::Instant::now(),
                ));
            } else {
                app.toast_message = Some((
                    "Slack: not connected.".to_string(),
                    std::time::Instant::now(),
                ));
            }
        }
        CommandAction::SlackStatus => {
            let status_msg = if let Some(ref slack_state_arc) = app.slack_state {
                if let Ok(state) = slack_state_arc.try_lock() {
                    let conn = match &state.status {
                        crate::slack::SlackConnectionStatus::Connected => "Connected",
                        crate::slack::SlackConnectionStatus::Disconnected => "Disconnected",
                        crate::slack::SlackConnectionStatus::Reconnecting => "Reconnecting...",
                        crate::slack::SlackConnectionStatus::AuthError(e) => {
                            if e.is_empty() {
                                "Auth Error"
                            } else {
                                "Auth Error (see logs)"
                            }
                        }
                    };
                    let threads = state.thread_sessions.len();
                    let buffers = state.response_buffers.len();
                    let m = &state.metrics;
                    format!(
                        "Slack: {} | routed:{} fail:{} replies:{} batches:{} reconnect:{} | {} threads | {} pending",
                        conn, m.messages_routed, m.triage_failures, m.thread_replies,
                        m.batches_sent, m.reconnections, threads, buffers
                    )
                } else {
                    "Slack: state locked (busy)".to_string()
                }
            } else {
                "Slack: not initialized".to_string()
            };
            app.toast_message = Some((status_msg, std::time::Instant::now()));
        }
        CommandAction::SlackLogs => {
            app.show_slack_log = !app.show_slack_log;
            app.slack_log_scroll = 0;
        }
        _ => {}
    }
    Ok(())
}
