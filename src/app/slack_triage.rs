use crate::app::App;
use tracing::{error, info};

impl App {
    /// Handle `SlackBackgroundEvent::IncomingMessage` — top-level message triage.
    pub(super) fn handle_incoming_message(
        &mut self,
        text: String,
        channel: String,
        ts: String,
        _user: String,
    ) {
        info!("Slack: incoming top-level message, starting triage");
        if let Some(ref state) = self.slack_state {
            let st = state.clone();
            let preview = text.chars().take(80).collect::<String>();
            tokio::spawn(async move {
                st.lock().await.log(
                    crate::slack::SlackLogLevel::Info,
                    format!("Incoming message: {}...", preview),
                );
            });
        }

        // ── @ command interception (bypass triage) ──────────────
        let trimmed = text.trim();
        if trimmed.starts_with("@list-projects") || trimmed.starts_with("@session ") {
            self.handle_at_command_intercept(&text, &channel, &ts);
            return;
        }

        // Spawn triage: send the message to the triage session for project detection.
        let projects: Vec<(String, String)> = self
            .projects
            .iter()
            .filter(|p| p.name != "slack-triage")
            .map(|p| (p.name.clone(), p.path.to_string_lossy().to_string()))
            .collect();
        let triage_sessions: Vec<crate::slack::SessionMeta> = self.collect_session_meta();
        let prompt = crate::slack::build_triage_prompt(&projects, &triage_sessions, &text);
        let bg_tx = self.bg_tx.clone();
        let base_url = crate::app::base_url().to_string();
        let original_text = text.clone();

        if let Ok(triage_dir) = crate::slack::triage_project_dir() {
            let triage_dir_str = triage_dir.to_string_lossy().to_string();
            tokio::spawn(async move {
                Self::run_triage(
                    bg_tx, base_url, triage_dir_str, prompt, ts, channel, original_text,
                )
                .await;
            });
        } else {
            error!("Slack: triage_project_dir() failed, cannot route message");
            if let Some(ref auth) = self.slack_auth {
                let bot_token = auth.bot_token.clone();
                let ch = channel.clone();
                let tts = ts.clone();
                tokio::spawn(async move {
                    let client = reqwest::Client::new();
                    let _ = crate::slack::post_message(
                        &client,
                        &bot_token,
                        &ch,
                        "opman: Internal error — could not locate triage project directory. \
                         Please check your config.",
                        Some(&tts),
                    )
                    .await;
                });
            }
        }
    }

    /// Handle `@list-projects` and `@session` commands from an incoming message.
    fn handle_at_command_intercept(&mut self, text: &str, channel: &str, ts: &str) {
        let trimmed = text.trim();
        let bot_token = self
            .slack_auth
            .as_ref()
            .map(|a| a.bot_token.clone())
            .unwrap_or_default();
        let base_url = crate::app::base_url().to_string();
        let buffer_secs = self.config.settings.slack.relay_buffer_secs;
        let slack_st = self.slack_state.clone();

        if trimmed == "@list-projects" || trimmed == "@list-projects " {
            let projects: Vec<(String, String)> = self
                .projects
                .iter()
                .filter(|p| p.name != "slack-triage")
                .map(|p| (p.name.clone(), p.path.to_string_lossy().to_string()))
                .collect();
            let ch = channel.to_string();
            let tts = ts.to_string();
            tokio::spawn(async move {
                crate::slack::handle_list_projects_command(&projects, &ch, &tts, &bot_token).await;
            });
        } else if trimmed.starts_with("@session ") {
            let rest = trimmed.strip_prefix("@session ").unwrap_or("").trim();
            let (session_query, message_text) = if rest.starts_with('"') {
                if let Some(end_quote) = rest[1..].find('"') {
                    let name = &rest[1..1 + end_quote];
                    let msg = rest[1 + end_quote + 1..].trim();
                    (name.to_string(), msg.to_string())
                } else {
                    (rest.to_string(), String::new())
                }
            } else {
                match rest.split_once(' ') {
                    Some((q, m)) => (q.to_string(), m.to_string()),
                    None => (rest.to_string(), String::new()),
                }
            };

            let all_sessions: Vec<crate::slack::SessionMeta> = self.collect_session_meta();
            let ch = channel.to_string();
            let tts = ts.to_string();
            tokio::spawn(async move {
                if let Some(ref st) = slack_st {
                    crate::slack::handle_session_command(
                        &session_query,
                        &message_text,
                        &all_sessions,
                        &ch,
                        &tts,
                        &bot_token,
                        &base_url,
                        buffer_secs,
                        st.clone(),
                    )
                    .await;
                }
            });
        }
    }

    /// Collect session metadata for all non-triage projects.
    pub(super) fn collect_session_meta(&self) -> Vec<crate::slack::SessionMeta> {
        self.projects
            .iter()
            .enumerate()
            .filter(|(_, p)| p.name != "slack-triage")
            .flat_map(|(idx, p)| {
                let pname = p.name.clone();
                let pdir = p.path.to_string_lossy().to_string();
                p.sessions.iter().map(move |s| crate::slack::SessionMeta {
                    id: s.id.clone(),
                    title: s.title.clone(),
                    parent_id: s.parent_id.clone(),
                    updated: s.time.updated,
                    project_idx: idx,
                    project_name: pname.clone(),
                    project_dir: pdir.clone(),
                })
            })
            .collect()
    }
}
