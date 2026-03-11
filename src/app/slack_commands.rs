use crate::app::App;
use tracing::{error, info};

impl App {
    /// Handle `SlackBackgroundEvent::SlashCommand`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_slash_command_event(
        &mut self,
        command: String,
        text: String,
        channel: String,
        _user: String,
        response_url: String,
        trigger_id: String,
    ) {
        info!("Slack slash command: {} {}", command, text);
        if let Some(ref state) = self.slack_state {
            let st = state.clone();
            let cmd_log = command.clone();
            let text_log = text.clone();
            tokio::spawn(async move {
                st.lock().await.log(
                    crate::slack::SlackLogLevel::Info,
                    format!("Slash command: {} {}", cmd_log, text_log),
                );
            });
        }

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

        let cmd = command.clone();
        let txt = text.clone();
        let ch = channel.clone();
        let rurl = response_url.clone();
        let trig = trigger_id.clone();

        tokio::spawn(async move {
            use crate::slack::SlashCommandOutcome;

            let outcome = crate::slack::handle_slash_command(
                &cmd, &txt, &ch, &rurl, &trig, &projects, &sessions, &bot_token,
            )
            .await;

            match outcome {
                SlashCommandOutcome::Handled => {}
                SlashCommandOutcome::NeedsTriage {
                    triage_text,
                    force_connect,
                    force_route,
                } => {
                    let full_cmd = if txt.is_empty() {
                        cmd.clone()
                    } else {
                        format!("{} {}", cmd, txt)
                    };
                    let client = reqwest::Client::new();
                    let ts = match crate::slack::post_message(
                        &client,
                        &bot_token,
                        &ch,
                        &format!(":hourglass_flowing_sand: Processing `{}`...", full_cmd),
                        None,
                    )
                    .await
                    {
                        Ok(ts) => ts,
                        Err(e) => {
                            error!("Slash command: failed to post processing msg: {}", e);
                            return;
                        }
                    };

                    Self::run_slash_triage(
                        bg_tx,
                        base_url,
                        bot_token,
                        projects,
                        sessions,
                        triage_text,
                        txt,
                        ts,
                        ch,
                        force_connect,
                        force_route,
                    )
                    .await;
                }
            }
        });
    }

}
