use crate::app::App;

impl App {
    /// Main dispatcher for Slack background events.
    ///
    /// Each variant is handled by a dedicated helper method defined in
    /// the sibling `slack_*` modules to keep individual files small.
    pub(super) fn handle_slack_event(&mut self, event: crate::slack::SlackBackgroundEvent) {
        use crate::slack::SlackBackgroundEvent;

        match event {
            SlackBackgroundEvent::ConnectionStatus(status) => {
                self.handle_connection_status(status);
            }
            SlackBackgroundEvent::IncomingMessage {
                text,
                channel,
                ts,
                user,
            } => {
                self.handle_incoming_message(text, channel, ts, user);
            }
            SlackBackgroundEvent::TriageResult {
                thread_ts,
                channel,
                original_text,
                rewritten_query,
                project_path,
                model,
                direct_answer,
                create_session,
                connect_only,
                error,
            } => {
                self.handle_triage_result(
                    thread_ts,
                    channel,
                    original_text,
                    rewritten_query,
                    project_path,
                    model,
                    direct_answer,
                    create_session,
                    connect_only,
                    error,
                );
            }
            SlackBackgroundEvent::IncomingThreadReply {
                text,
                channel,
                ts,
                thread_ts,
                user,
            } => {
                self.handle_incoming_thread_reply(text, channel, ts, thread_ts, user);
            }
            SlackBackgroundEvent::ResponseBatch {
                channel,
                thread_ts,
                text,
            } => {
                self.handle_response_batch(channel, thread_ts, text);
            }
            SlackBackgroundEvent::BlockAction {
                action_id,
                channel,
                message_ts,
                thread_ts,
                user,
            } => {
                self.handle_block_action(action_id, channel, message_ts, thread_ts, user);
            }
            SlackBackgroundEvent::SlashCommand {
                command,
                text,
                channel,
                user,
                response_url,
                trigger_id,
            } => {
                self.handle_slash_command_event(
                    command,
                    text,
                    channel,
                    user,
                    response_url,
                    trigger_id,
                );
            }
            SlackBackgroundEvent::ViewSubmission {
                callback_id,
                user,
                values,
                private_metadata,
                trigger_id,
            } => {
                self.handle_view_submission(
                    callback_id,
                    user,
                    values,
                    private_metadata,
                    trigger_id,
                );
            }
            SlackBackgroundEvent::OAuthComplete(result) => {
                self.handle_oauth_complete(result);
            }
        }
    }
}
