//! Live relay watcher – polls an OpenCode session for new messages and relays
//! them to a Slack thread using streaming with `task_update` chunks.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Notify};
use tracing::info;

use super::super::auth::SlackSessionMap;
use super::super::state::SlackState;
use super::super::tools::fetch_session_messages_with_tools;
use super::content::build_relay_content;
use super::dispatch::dispatch_relay;
use super::lifecycle::{handle_idle_stop, maybe_rotate_stream};

/// Number of consecutive idle polls before we finalize an active stream.
const IDLE_STOP_THRESHOLD: u32 = 3;

/// Spawn a background task that polls a session for new messages every
/// `buffer_secs` and relays them to a Slack thread using streaming with
/// `task_update` chunks for tool progress.
///
/// Returns a `JoinHandle` whose `AbortHandle` can be stored to cancel later.
pub fn spawn_session_relay_watcher(
    session_id: String,
    project_dir: String,
    channel: String,
    thread_ts: String,
    bot_token: String,
    base_url: String,
    buffer_secs: u64,
    slack_state: Arc<Mutex<SlackState>>,
) -> tokio::task::JoinHandle<()> {
    spawn_session_relay_watcher_labeled(
        session_id,
        project_dir,
        channel,
        thread_ts,
        bot_token,
        base_url,
        buffer_secs,
        slack_state,
        None,
    )
}

/// Spawn a relay watcher with an optional label prefix for the first streamed
/// message (e.g. ":robot_face: **Subagent:**" for child sessions).
pub fn spawn_session_relay_watcher_labeled(
    session_id: String,
    project_dir: String,
    channel: String,
    thread_ts: String,
    bot_token: String,
    base_url: String,
    buffer_secs: u64,
    slack_state: Arc<Mutex<SlackState>>,
    label: Option<String>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let fallback_interval = Duration::from_secs(buffer_secs.max(1));
        let debounce = Duration::from_millis(800);
        let mut idle_polls: u32 = 0;
        let mut last_streamed_role: Option<String> = None;
        let mut label_emitted = false;
        let mut stream_started_at: Option<tokio::time::Instant> = None;
        let mut accumulated_stream_md = String::new();
        let mut accumulated_raw_md = String::new();

        // Register a Notify so SSE message events can wake us up.
        let notify = {
            let mut s = slack_state.lock().await;
            let n = s
                .relay_notifiers
                .entry(session_id.clone())
                .or_insert_with(|| Arc::new(Notify::new()))
                .clone();
            n
        };

        info!(
            "Slack relay watcher started for session {} (poll every {}s, debounce {}ms, label={:?})",
            &session_id[..8.min(session_id.len())],
            buffer_secs,
            debounce.as_millis(),
            label
        );

        loop {
            // Wait for either the fallback timer or an SSE-driven notification.
            tokio::select! {
                _ = tokio::time::sleep(fallback_interval) => {}
                _ = notify.notified() => {
                    tokio::time::sleep(debounce).await;
                }
            }

            // ── External stop request (from SseSessionIdle) ─────────
            {
                let mut s = slack_state.lock().await;
                if s.stream_stop_requested.remove(&session_id) {
                    if let Some(ts) = s.streaming_messages.remove(&session_id) {
                        drop(s);
                        tracing::info!(
                            "Slack relay: honoring external stop request for stream {} session {}",
                            ts,
                            &session_id[..8.min(session_id.len())]
                        );
                        if let Err(e) =
                            super::super::api::stop_stream(&client, &bot_token, &channel, &ts)
                                .await
                        {
                            tracing::warn!(
                                "Slack relay: stopStream (external request) failed: {}",
                                e
                            );
                        }
                        accumulated_stream_md.clear();
                        accumulated_raw_md.clear();
                        stream_started_at = None;
                    }
                }
            }

            // Read current offset.
            let offset = {
                let s = slack_state.lock().await;
                s.session_msg_offset.get(&session_id).copied().unwrap_or(0)
            };

            // Fetch messages with structured tool data from the OpenCode API.
            let messages = match fetch_session_messages_with_tools(
                &client,
                &base_url,
                &project_dir,
                &session_id,
            )
            .await
            {
                Ok(msgs) => msgs,
                Err(e) => {
                    tracing::warn!(
                        "Slack relay watcher: failed to fetch messages for session {}: {}",
                        &session_id[..8.min(session_id.len())],
                        e
                    );
                    continue;
                }
            };

            let total_count = messages.len();
            if total_count <= offset {
                idle_polls += 1;
                if idle_polls >= IDLE_STOP_THRESHOLD {
                    handle_idle_stop(
                        &client,
                        &bot_token,
                        &channel,
                        &session_id,
                        idle_polls,
                        &slack_state,
                        &mut accumulated_stream_md,
                        &mut accumulated_raw_md,
                        &mut stream_started_at,
                    )
                    .await;
                }
                continue;
            }

            // We have new messages -- reset idle counter.
            idle_polls = 0;

            let new_messages: Vec<_> = messages
                .into_iter()
                .skip(offset)
                .filter(|m| !m.text.is_empty() || !m.tools.is_empty())
                .collect();

            if new_messages.is_empty() {
                let mut s = slack_state.lock().await;
                s.session_msg_offset.insert(session_id.clone(), total_count);
                continue;
            }

            tracing::info!(
                "Slack relay watcher: relaying {} new message(s) for session {} to thread {}",
                new_messages.len(),
                &session_id[..8.min(session_id.len())],
                thread_ts
            );

            // Build relay text and task chunks from new messages.
            let (mut relay_text, relay_text_raw, all_task_chunks, groups) =
                build_relay_content(
                    &new_messages,
                    &session_id,
                    &last_streamed_role,
                    &label,
                    label_emitted,
                    &slack_state,
                )
                .await;

            // Prepend optional label on first relay.
            if !label_emitted {
                if let Some(ref lbl) = label {
                    relay_text = format!("{}\n{}", lbl, relay_text);
                }
                label_emitted = true;
            }
            if let Some((role, _)) = groups.last() {
                last_streamed_role = Some(role.clone());
            }

            let has_task_chunks = !all_task_chunks.is_empty();

            // Check if we already have an active stream for this session.
            let mut active_stream_ts = {
                let s = slack_state.lock().await;
                s.streaming_messages.get(&session_id).cloned()
            };

            // ── Stream rotation ─────────────────────────────────────
            maybe_rotate_stream(
                &client,
                &bot_token,
                &channel,
                &session_id,
                &new_messages,
                &relay_text,
                offset,
                total_count,
                &slack_state,
                &mut active_stream_ts,
                &mut accumulated_stream_md,
                &mut accumulated_raw_md,
                &mut stream_started_at,
            )
            .await;

            // ── Dispatch to segment or stream relay ─────────────────
            dispatch_relay(
                &client,
                &bot_token,
                &channel,
                &thread_ts,
                &session_id,
                &relay_text,
                &relay_text_raw,
                &all_task_chunks,
                has_task_chunks,
                active_stream_ts.as_deref(),
                offset,
                total_count,
                &slack_state,
                &mut accumulated_stream_md,
                &mut accumulated_raw_md,
                &mut stream_started_at,
            )
            .await;

            // Update offset and persist the session map.
            {
                let mut s = slack_state.lock().await;
                s.session_msg_offset.insert(session_id.clone(), total_count);
                let map = SlackSessionMap {
                    session_threads: s.session_threads.clone(),
                    thread_sessions: s.thread_sessions.clone(),
                    msg_offsets: s.session_msg_offset.clone(),
                };
                if let Err(e) = map.save() {
                    tracing::warn!("Slack relay watcher: failed to save session map: {}", e);
                }
            }
        }
    })
}


