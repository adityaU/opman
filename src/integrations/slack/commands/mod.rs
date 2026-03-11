//! Top-level @ commands and thread slash commands for Slack.

mod command_api;
mod info;
mod model;
mod modals;
mod session_control;
mod session_route;
mod slash;
mod test_blockkit;
mod test_blockkit_ext;
mod toplevel;

// Re-export public API.
pub use modals::open_connect_session_modal;
pub use slash::{handle_sessions_slash, handle_slash_command, SlashCommandOutcome};
pub use toplevel::{handle_list_projects_command, handle_session_command};

use std::sync::Arc;

use tokio::sync::Mutex;

use super::state::SlackState;

use command_api::{do_command_api, do_passthrough_command};
use info::{do_help_command, do_messages_command, do_status_command, do_todos_command};
use model::do_model_command;
use session_control::{do_compact_command, do_detach_command, do_stop_command, do_watcher_command};
use test_blockkit::do_test_blockkit_command;

// ── Slack Thread Slash Commands ─────────────────────────────────────────

/// Handle a slash command sent in a Slack thread.
///
/// Returns `true` if the text was recognized as a command (and handled),
/// `false` if it should be treated as a normal thread reply.
pub async fn handle_thread_slash_command(
    text: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_idx: usize,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    slack_state: &Arc<Mutex<SlackState>>,
    watcher_inserted: bool,
    watcher_removed: bool,
) -> bool {
    let trimmed = text.trim();
    let (cmd, args) = match trimmed.split_once(char::is_whitespace) {
        Some((c, a)) => (c, a.trim()),
        None => (trimmed, ""),
    };

    match cmd {
        "@stop" => {
            do_stop_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                slack_state,
            )
            .await;
            true
        }
        "@watcher" => {
            do_watcher_command(
                channel,
                thread_ts,
                session_id,
                project_idx,
                project_dir,
                bot_token,
                base_url,
                slack_state,
                watcher_inserted,
                watcher_removed,
                args,
            )
            .await;
            true
        }
        "@compact" | "@summarize" => {
            do_compact_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
            )
            .await;
            true
        }
        "@status" => {
            do_status_command(
                channel,
                thread_ts,
                session_id,
                project_idx,
                project_dir,
                bot_token,
                base_url,
                slack_state,
            )
            .await;
            true
        }
        "@todos" => {
            do_todos_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
            )
            .await;
            true
        }
        "@detach" => {
            do_detach_command(channel, thread_ts, session_id, bot_token, slack_state).await;
            true
        }
        "@messages" => {
            do_messages_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                args,
            )
            .await;
            true
        }
        "@undo" => {
            do_command_api(
                "undo",
                "",
                None,
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                ":leftwards_arrow_with_hook: Undo triggered.",
                ":x: Undo failed",
            )
            .await;
            true
        }
        "@redo" => {
            do_command_api(
                "redo",
                "",
                None,
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                ":arrow_right_hook: Redo triggered.",
                ":x: Redo failed",
            )
            .await;
            true
        }
        "@model" | "@models" => {
            do_model_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                args,
            )
            .await;
            true
        }
        "@export" => {
            do_command_api(
                "export",
                "",
                None,
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                ":outbox_tray: Session exported.",
                ":x: Export failed",
            )
            .await;
            true
        }
        "@test_blockkit" => {
            do_test_blockkit_command(channel, thread_ts, bot_token).await;
            true
        }
        "@help" => {
            do_help_command(channel, thread_ts, bot_token).await;
            true
        }
        _ => {
            // Unrecognized @ command — try passthrough via command API,
            // fall back to help if the server rejects it.
            let oc_cmd = cmd.strip_prefix('@').unwrap_or(cmd);
            do_passthrough_command(
                oc_cmd,
                args,
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
            )
            .await;
            true
        }
    }
}
