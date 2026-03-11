use crate::app::{App, PendingSlackMessage};
use tracing::{info, warn};

impl App {
    /// Handle `SlackBackgroundEvent::TriageResult`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_triage_result(
        &mut self,
        thread_ts: String,
        channel: String,
        original_text: String,
        rewritten_query: Option<String>,
        project_path: Option<String>,
        _model: Option<String>,
        direct_answer: Option<String>,
        create_session: bool,
        connect_only: bool,
        error: Option<String>,
    ) {
        if let Some(ref rq) = rewritten_query {
            info!(
                "Slack triage: rewritten query: \"{}\"",
                rq.chars().take(120).collect::<String>()
            );
        }

        // Handle errors.
        if let Some(err) = error {
            self.handle_triage_error(&thread_ts, &channel, &err);
            return;
        }

        // Handle direct answers from triage AI (informational queries).
        if let Some(answer) = direct_answer {
            self.handle_triage_direct_answer(&thread_ts, &channel, &answer);
            return;
        }

        // Handle explicit session creation requests (Action C).
        if create_session {
            self.handle_triage_create_session(
                &thread_ts,
                &channel,
                &original_text,
                &rewritten_query,
                &project_path,
            );
            return;
        }

        if let Some(ref path) = project_path {
            self.handle_triage_route_to_project(
                &thread_ts,
                &channel,
                &original_text,
                &rewritten_query,
                path,
                connect_only,
            );
        } else {
            // Triage AI couldn't determine which project the user meant.
            warn!("Slack: triage returned no project_path for message");
            self.post_no_project_error(&channel, &thread_ts);
        }
    }

    /// Post a triage error back to Slack.
    fn handle_triage_error(&mut self, thread_ts: &str, channel: &str, err: &str) {
        warn!("Slack triage error: {}", err);
        if let Some(ref state) = self.slack_state {
            let st = state.clone();
            let err_msg = err.to_string();
            tokio::spawn(async move {
                let mut s = st.lock().await;
                s.metrics.triage_failures += 1;
                s.log(
                    crate::slack::SlackLogLevel::Error,
                    format!("Triage failed: {}", err_msg),
                );
            });
        }
        if let Some(ref auth) = self.slack_auth {
            let bot_token = auth.bot_token.clone();
            let ch = channel.to_string();
            let tts = thread_ts.to_string();
            let msg = format!("opman: {}", err);
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                let _ =
                    crate::slack::post_message(&client, &bot_token, &ch, &msg, Some(&tts)).await;
            });
        }
    }

    /// Post a direct answer from the triage AI.
    fn handle_triage_direct_answer(&mut self, thread_ts: &str, channel: &str, answer: &str) {
        info!("Slack triage: direct answer ({} chars)", answer.len());
        if let Some(ref state) = self.slack_state {
            let st = state.clone();
            tokio::spawn(async move {
                let mut s = st.lock().await;
                s.metrics.messages_routed += 1;
                s.metrics.last_routed_at = Some(std::time::Instant::now());
                s.log(
                    crate::slack::SlackLogLevel::Info,
                    "Triage answered informational query directly".to_string(),
                );
            });
        }
        if let Some(ref auth) = self.slack_auth {
            let bot_token = auth.bot_token.clone();
            let ch = channel.to_string();
            let tts = thread_ts.to_string();
            let answer = answer.to_string();
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                let _ = crate::slack::post_message(&client, &bot_token, &ch, &answer, Some(&tts))
                    .await;
            });
        }
    }

    /// Handle triage Action C: create a new session.
    fn handle_triage_create_session(
        &mut self,
        thread_ts: &str,
        channel: &str,
        original_text: &str,
        rewritten_query: &Option<String>,
        project_path: &Option<String>,
    ) {
        if let Some(ref path) = project_path {
            let project_idx = self
                .projects
                .iter()
                .position(|p| p.path.to_string_lossy() == path.as_str());

            if let Some(pidx) = project_idx {
                let project = &self.projects[pidx];
                info!(
                    "Slack triage: user requested new session in project \"{}\" (idx={})",
                    project.name, pidx
                );

                self.pending_slack_messages.push(PendingSlackMessage {
                    project_idx: pidx,
                    thread_ts: thread_ts.to_string(),
                    channel: channel.to_string(),
                    original_text: original_text.to_string(),
                    rewritten_query: rewritten_query.clone(),
                });
                self.pending_new_session = Some(pidx);

                if let Some(ref auth) = self.slack_auth {
                    let bot_token = auth.bot_token.clone();
                    let ch = channel.to_string();
                    let tts = thread_ts.to_string();
                    let pname = project.name.clone();
                    tokio::spawn(async move {
                        let client = reqwest::Client::new();
                        let msg = format!(
                            ":hourglass_flowing_sand: Creating a new session in *{}*\u{2026}",
                            pname
                        );
                        let _ = crate::slack::post_message(
                            &client, &bot_token, &ch, &msg, Some(&tts),
                        )
                        .await;
                    });
                }
            } else {
                warn!(
                    "Slack: create_session requested but no project found at path {:?}",
                    path
                );
                self.post_project_not_found_error(channel, thread_ts, path);
            }
        } else {
            warn!("Slack: create_session requested but no project_path in triage result");
            if let Some(ref auth) = self.slack_auth {
                let bot_token = auth.bot_token.clone();
                let ch = channel.to_string();
                let tts = thread_ts.to_string();
                let available: Vec<String> = self
                    .projects
                    .iter()
                    .filter(|p| p.name != "slack-triage")
                    .map(|p| format!("\"{}\"", p.name))
                    .collect();
                tokio::spawn(async move {
                    let client = reqwest::Client::new();
                    let msg = format!(
                        "opman: Please specify which project to create a session in. \
                         Available projects: {}",
                        available.join(", ")
                    );
                    let _ =
                        crate::slack::post_message(&client, &bot_token, &ch, &msg, Some(&tts))
                            .await;
                });
            }
        }
    }

    /// Post "project not found" error with available projects list.
    pub(super) fn post_project_not_found_error(
        &self,
        channel: &str,
        thread_ts: &str,
        path: &str,
    ) {
        if let Some(ref auth) = self.slack_auth {
            let bot_token = auth.bot_token.clone();
            let ch = channel.to_string();
            let tts = thread_ts.to_string();
            let err_path = path.to_string();
            let available: Vec<String> = self
                .projects
                .iter()
                .filter(|p| p.name != "slack-triage")
                .map(|p| format!("\"{}\" ({})", p.name, p.path.to_string_lossy()))
                .collect();
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                let msg = format!(
                    "opman: Could not find project at path \"{}\". \
                     Available projects: {}",
                    err_path,
                    if available.is_empty() {
                        "(none)".to_string()
                    } else {
                        available.join(", ")
                    }
                );
                let _ =
                    crate::slack::post_message(&client, &bot_token, &ch, &msg, Some(&tts)).await;
            });
        }
    }

    /// Post "no project determined" error.
    fn post_no_project_error(&self, channel: &str, thread_ts: &str) {
        if let Some(ref auth) = self.slack_auth {
            let bot_token = auth.bot_token.clone();
            let ch = channel.to_string();
            let tts = thread_ts.to_string();
            let available: Vec<String> = self
                .projects
                .iter()
                .filter(|p| p.name != "slack-triage")
                .map(|p| p.name.clone())
                .collect();
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                let msg = format!(
                    "opman: Could not determine which project you're referring to. \
                     Please mention the project name explicitly. \
                     Available projects: {}",
                    if available.is_empty() {
                        "(none)".to_string()
                    } else {
                        available.join(", ")
                    }
                );
                let _ =
                    crate::slack::post_message(&client, &bot_token, &ch, &msg, Some(&tts)).await;
            });
        }
    }
}
