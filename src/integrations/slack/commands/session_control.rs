//! Session control commands: stop, watcher, compact, detach.

use std::sync::Arc;

use tokio::sync::Mutex;

use super::super::api::post_message;
use super::super::relay::spawn_session_relay_watcher;
use super::super::state::SlackState;
use super::command_api::do_command_api;

/// `@stop` — Cancel the running OpenCode session (abort LLM generation and tool
/// execution).  The relay watcher and stream are left intact so that the final
/// state is still delivered to Slack once the session becomes idle.
pub(super) async fn do_stop_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    _slack_state: &Arc<Mutex<SlackState>>,
) {
    let client = reqwest::Client::new();

    // Abort the OpenCode session via server API.
    // This calls POST /session/{id}/abort which cancels any running LLM
    // generation and tool execution on the server side.
    let api = crate::api::ApiClient::new();
    let msg = match api.abort_session(base_url, project_dir, session_id).await {
        Ok(()) => {
            tracing::info!(
                "Slack @stop: aborted session {} via API",
                &session_id[..8.min(session_id.len())]
            );
            ":octagonal_sign: Session interrupted. The relay watcher will deliver any remaining output.".to_string()
        }
        Err(e) => {
            tracing::warn!(
                "Slack @stop: failed to abort session {}: {}",
                &session_id[..8.min(session_id.len())],
                e
            );
            format!(":warning: Failed to stop session: {}", e)
        }
    };
    let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
}

/// `@watcher` — Start or stop a continuation + hang protection watcher for this session.
///
/// The actual `WatcherConfig` insertion/removal happens inline in `app.rs`
/// (synchronous context with `&mut self` access).  The `watcher_inserted` and
/// `watcher_removed` flags tell us what happened so we can post the right
/// confirmation to Slack.  `args` is the subcommand text after `@watcher`
/// (e.g. "stop", "off", "remove", or "").
pub(super) async fn do_watcher_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    _project_idx: usize,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    slack_state: &Arc<Mutex<SlackState>>,
    watcher_inserted: bool,
    watcher_removed: bool,
    args: &str,
) {
    let client = reqwest::Client::new();

    let is_stop = matches!(args, "stop" | "off" | "remove");

    // --- Watcher removal case (`@watcher stop` / `off` / `remove`) ---
    if watcher_removed {
        let msg = ":no_entry_sign: Watcher removed for this thread.";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
        return;
    }

    // User asked to stop but no watcher was active.
    if is_stop {
        let msg = ":information_source: No watcher is active for this thread.";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
        return;
    }

    // --- Watcher insertion case (`@watcher` with no subcommand) ---
    if watcher_inserted {
        let msg =
            ":eyes: Watcher enabled for this thread.\n• Idle timeout: 15s\n• Hang detection: 180s";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
    } else {
        let msg = ":warning: Could not enable watcher — failed to acquire lock on session watchers. Please try again.";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
        return;
    }

    // Also ensure the relay watcher is running.
    {
        let s = slack_state.lock().await;
        if !s.relay_abort_handles.contains_key(session_id) {
            drop(s);
            // Need to re-spawn relay watcher.
            let handle = spawn_session_relay_watcher(
                session_id.to_string(),
                project_dir.to_string(),
                channel.to_string(),
                thread_ts.to_string(),
                bot_token.to_string(),
                base_url.to_string(),
                3,
                slack_state.clone(),
            );
            let mut s = slack_state.lock().await;
            s.relay_abort_handles
                .insert(session_id.to_string(), handle.abort_handle());
            tracing::info!(
                "Slack @watcher: re-spawned relay watcher for session {}",
                &session_id[..8.min(session_id.len())]
            );
        }
    }
}

/// `@compact` / `@summarize` — Compact/summarize the current session via the
/// OpenCode command API (`POST /session/:id/command { command: "compact" }`).
pub(super) async fn do_compact_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
) {
    do_command_api(
        "compact",
        "",
        None,
        channel,
        thread_ts,
        session_id,
        project_dir,
        bot_token,
        base_url,
        ":recycle: Compaction triggered — session will summarize and continue.",
        ":x: Compaction failed",
    )
    .await;
}

/// `@detach` — Disconnect the relay watcher from this thread.
pub(super) async fn do_detach_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    bot_token: &str,
    slack_state: &Arc<Mutex<SlackState>>,
) {
    let client = reqwest::Client::new();

    let mut s = slack_state.lock().await;
    if let Some(_old_sid) = s.detach_relay(thread_ts) {
        drop(s);
        let msg = format!(
            ":wave: Detached relay for session `{}` from this thread. Messages will no longer be relayed here.",
            &session_id[..8.min(session_id.len())]
        );
        let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
        tracing::info!(
            "Slack @detach: detached session {} from thread",
            &session_id[..8.min(session_id.len())]
        );
    } else {
        drop(s);
        let _ = post_message(
            &client,
            bot_token,
            channel,
            ":information_source: No relay is attached to this thread.",
            Some(thread_ts),
        )
        .await;
    }
}
