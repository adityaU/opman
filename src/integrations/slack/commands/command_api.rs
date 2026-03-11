//! Generic command API helper and passthrough command.

use super::super::api::post_message;
use super::info::do_help_command;

// ── Generic Command API Helper ─────────────────────────────────────────

/// Execute an OpenCode slash command via `POST /session/:id/command` and post
/// a success or failure message to the Slack thread.
pub(super) async fn do_command_api(
    command: &str,
    arguments: &str,
    model: Option<&str>,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    success_msg: &str,
    error_prefix: &str,
) {
    let client = reqwest::Client::new();
    let api = crate::api::ApiClient::new();

    match api
        .execute_session_command(base_url, project_dir, session_id, command, arguments, model)
        .await
    {
        Ok(_resp) => {
            let _ = post_message(&client, bot_token, channel, success_msg, Some(thread_ts)).await;
            tracing::info!(
                "Slack @{}: executed for session {}",
                command,
                &session_id[..8.min(session_id.len())]
            );
        }
        Err(e) => {
            let msg = format!("{}: {}", error_prefix, e);
            let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
            tracing::warn!(
                "Slack @{}: failed for session {}: {}",
                command,
                &session_id[..8.min(session_id.len())],
                e
            );
        }
    }
}

/// Passthrough: attempt to execute an unrecognized `@<command>` as a custom
/// OpenCode command via the command API. If the server rejects it (404 or error),
/// show the `@help` output instead.
pub(super) async fn do_passthrough_command(
    command: &str,
    arguments: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
) {
    let api = crate::api::ApiClient::new();

    match api
        .execute_session_command(base_url, project_dir, session_id, command, arguments, None)
        .await
    {
        Ok(_) => {
            let client = reqwest::Client::new();
            let msg = format!(":white_check_mark: `@{}` executed.", command);
            let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
            tracing::info!(
                "Slack passthrough @{}: executed for session {}",
                command,
                &session_id[..8.min(session_id.len())]
            );
        }
        Err(e) => {
            tracing::debug!(
                "Slack passthrough @{}: rejected for session {}: {}",
                command,
                &session_id[..8.min(session_id.len())],
                e
            );
            // Unknown command — show help.
            do_help_command(channel, thread_ts, bot_token).await;
        }
    }
}
