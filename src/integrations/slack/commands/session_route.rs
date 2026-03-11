//! Session routing continuation — processes AI triage match result and connects relay.

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use super::super::api::{post_message, send_user_message};
use super::super::auth::SlackSessionMap;
use super::super::relay::spawn_session_relay_watcher;
use super::super::state::SlackState;
use super::super::types::{SessionMeta, SlackLogLevel};

/// Continuation of `handle_session_command` after JSON parsing.
///
/// Validates the AI match, connects the relay, sends the user message, and
/// spawns the relay watcher.
pub(super) async fn handle_session_command_cont(
    client: &reqwest::Client,
    parsed: &serde_json::Value,
    all_sessions: &[SessionMeta],
    channel: &str,
    ts: &str,
    bot_token: &str,
    base_url: &str,
    buffer_secs: u64,
    message_text: &str,
    slack_state: Arc<Mutex<SlackState>>,
) {
    if let Some(err) = parsed
        .get("error")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let mut msg = format!(":warning: {}", err);
        if let Some(candidates) = parsed.get("candidates").and_then(|v| v.as_array()) {
            let list: Vec<String> = candidates
                .iter()
                .filter_map(|c| c.as_str().map(|s| format!("• {}", s)))
                .collect();
            if !list.is_empty() {
                msg.push_str(&format!(
                    "\n\nDid you mean one of these?\n{}",
                    list.join("\n")
                ));
            }
        }
        let _ = post_message(client, bot_token, channel, &msg, Some(ts)).await;
        return;
    }

    // Extract matched session ID. The AI returns the full session ID.
    let matched_session_id_raw = parsed
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    if matched_session_id_raw.is_empty() {
        let msg = ":warning: AI could not match a session. Use `@list-sessions <project>` to find session names.";
        let _ = post_message(client, bot_token, channel, msg, Some(ts)).await;
        return;
    }

    // The AI might return a prefix; find the full session by prefix match.
    let matched_meta = all_sessions
        .iter()
        .find(|s| s.id == matched_session_id_raw || s.id.starts_with(matched_session_id_raw));

    let matched_meta = match matched_meta {
        Some(m) => m,
        None => {
            let msg = format!(
                ":warning: AI matched session ID `{}` but it was not found in the session list.",
                matched_session_id_raw
            );
            let _ = post_message(client, bot_token, channel, &msg, Some(ts)).await;
            return;
        }
    };

    let session_id = matched_meta.id.clone();
    let session_name = if matched_meta.title.is_empty() {
        format!("Session {}", &session_id[..8.min(session_id.len())])
    } else {
        matched_meta.title.clone()
    };
    let project_name = matched_meta.project_name.clone();
    let project_dir = matched_meta.project_dir.clone();
    let pidx = matched_meta.project_idx;

    {
        let mut s = slack_state.lock().await;
        // Detach any existing relay for this session on a *different* thread.
        if let Some((old_ts, _)) = s.detach_relay_by_session(&session_id) {
            info!(
                "@ command: detached relay for session {} from old thread {}",
                &session_id[..8.min(session_id.len())],
                old_ts
            );
        }
        // One relay per thread: detach old session before attaching new.
        if let Some(old_sid) = s.detach_relay(ts) {
            info!(
                "@ command: detached previous relay (session {}) from thread {}",
                &old_sid[..8.min(old_sid.len())],
                ts
            );
        }
    }

    // Record thread→session mapping and mark this as the active relay.
    {
        let mut s = slack_state.lock().await;
        s.thread_sessions
            .insert(ts.to_string(), (pidx, session_id.clone()));
        s.session_threads
            .insert(session_id.clone(), (channel.to_string(), ts.to_string()));
        s.active_relay.insert(ts.to_string(), session_id.clone());
        s.metrics.messages_routed += 1;
        s.metrics.last_routed_at = Some(std::time::Instant::now());
        s.log(
            SlackLogLevel::Info,
            format!(
                "@ command routed to project \"{}\" session {}",
                project_name,
                &session_id[..8.min(session_id.len())]
            ),
        );
    }

    // Record current message offset so relay only shows new messages.
    match super::super::tools::fetch_session_messages_with_tools(
        client,
        base_url,
        &project_dir,
        &session_id,
    )
    .await
    {
        Ok(msgs) => {
            let mut s = slack_state.lock().await;
            s.session_msg_offset.insert(session_id.clone(), msgs.len());
            tracing::debug!(
                "@ command: recorded msg offset {} for session {}",
                msgs.len(),
                session_id
            );
        }
        Err(e) => {
            warn!(
                "@ command: failed to fetch msg offset for session {}: {}",
                session_id, e
            );
        }
    }

    // If a message was provided, send it to the session.
    let has_message = !message_text.is_empty();
    if has_message {
        // Use the AI's rewritten query if available, otherwise fall back to raw message.
        let final_message = parsed
            .get("rewritten_query")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "null")
            .unwrap_or(message_text);

        // Send the user message to the session.
        match send_user_message(client, base_url, &project_dir, &session_id, final_message).await {
            Ok(()) => {
                info!("@ command: user message sent to session {}", session_id);
                let ack = format!(
                    "relayed to project: {}, session: {}",
                    project_name, session_name
                );
                let _ = post_message(client, bot_token, channel, &ack, Some(ts)).await;
            }
            Err(e) => {
                tracing::error!("@ command: failed to send message to session: {}", e);
                let msg = format!(":x: Failed to send message: {}", e);
                let _ = post_message(client, bot_token, channel, &msg, Some(ts)).await;
                return;
            }
        }
    } else {
        // Relay-only: acknowledge the attachment without sending a message.
        let ack = format!(
            ":link: attached to project: {}, session: {} (relay only, no message sent)",
            project_name, session_name
        );
        let _ = post_message(client, bot_token, channel, &ack, Some(ts)).await;
    }

    // Spawn relay watcher (always — we detached the old one above so this is fresh).
    let already_watching = {
        let s = slack_state.lock().await;
        s.relay_abort_handles.contains_key(&session_id)
    };
    if !already_watching {
        let handle = spawn_session_relay_watcher(
            session_id.clone(),
            project_dir,
            channel.to_string(),
            ts.to_string(),
            bot_token.to_string(),
            base_url.to_string(),
            buffer_secs,
            slack_state.clone(),
        );
        let mut s = slack_state.lock().await;
        s.relay_abort_handles
            .insert(session_id.clone(), handle.abort_handle());

        // Persist session map to disk.
        let map = SlackSessionMap {
            session_threads: s.session_threads.clone(),
            thread_sessions: s.thread_sessions.clone(),
            msg_offsets: s.session_msg_offset.clone(),
        };
        if let Err(e) = map.save() {
            warn!("@ command: failed to persist session map: {}", e);
        }
    }
}
