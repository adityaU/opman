use crate::app::App;
use tracing::{debug, info, warn};

impl App {
    /// Handle `SlackBackgroundEvent::ViewSubmission`.
    pub(super) fn handle_view_submission(
        &mut self,
        callback_id: String,
        _user: String,
        values: serde_json::Value,
        private_metadata: String,
        trigger_id: String,
    ) {
        info!("Slack view submission: callback_id={}", callback_id);
        if let Some(ref state) = self.slack_state {
            let st = state.clone();
            let cb_log = callback_id.clone();
            tokio::spawn(async move {
                st.lock().await.log(
                    crate::slack::SlackLogLevel::Info,
                    format!("Modal submitted: {}", cb_log),
                );
            });
        }

        match callback_id.as_str() {
            "opman_sessions_modal" => {
                self.handle_sessions_modal_submission(&values, &private_metadata);
            }
            "opman_connect_project_modal" => {
                self.handle_connect_project_modal_submission(
                    &values,
                    &private_metadata,
                    &trigger_id,
                );
            }
            "opman_connect_modal" => {
                self.handle_connect_modal_submission(&values, &private_metadata);
            }
            other => {
                debug!("Unhandled modal callback_id: {}", other);
            }
        }
    }

    /// Handle the sessions modal submission.
    fn handle_sessions_modal_submission(
        &mut self,
        values: &serde_json::Value,
        private_metadata: &str,
    ) {
        let selected_project = values
            .pointer(
                "/project_select_block/project_select_action/selected_option/value",
            )
            .and_then(|v| v.as_str())
            .unwrap_or("__all__")
            .to_string();

        let filter = if selected_project == "__all__" {
            String::new()
        } else {
            selected_project
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

        let channel = if !private_metadata.is_empty() {
            private_metadata.to_string()
        } else {
            String::new()
        };

        if !channel.is_empty() {
            tokio::spawn(async move {
                crate::slack::handle_sessions_slash(
                    &filter, &projects, &sessions, &channel, "", &bot_token,
                )
                .await;
            });
        } else {
            warn!("ViewSubmission (sessions_modal): no channel available to post results");
        }
    }

    /// Handle the connect-project modal submission (step 1: project picker).
    fn handle_connect_project_modal_submission(
        &mut self,
        values: &serde_json::Value,
        private_metadata: &str,
        trigger_id: &str,
    ) {
        let selected_project = values
            .pointer(
                "/project_select_block/project_select_action/selected_option/value",
            )
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let channel = if !private_metadata.is_empty() {
            private_metadata.to_string()
        } else {
            String::new()
        };

        if selected_project.is_empty() || channel.is_empty() || trigger_id.is_empty() {
            warn!(
                "ViewSubmission (connect_project_modal): missing project={}, channel={}, trigger_id={}",
                selected_project, channel, trigger_id
            );
        } else {
            let sessions: Vec<crate::slack::SessionMeta> = self.collect_session_meta();
            let bot_token = self
                .slack_auth
                .as_ref()
                .map(|a| a.bot_token.clone())
                .unwrap_or_default();

            let trig = trigger_id.to_string();
            let proj = selected_project.clone();
            let ch = channel.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::slack::open_connect_session_modal(
                    &trig, &ch, &proj, &sessions, &bot_token,
                )
                .await
                {
                    warn!("Failed to open connect session modal: {}", e);
                    let client = reqwest::Client::new();
                    let _ = crate::slack::post_message(
                        &client,
                        &bot_token,
                        &ch,
                        &format!(
                            ":x: Failed to open session picker for *{}*: {}",
                            proj, e
                        ),
                        None,
                    )
                    .await;
                }
            });
        }
    }
}
