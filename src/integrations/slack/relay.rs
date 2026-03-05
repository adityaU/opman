//! Session relay watcher and response batching.
//!
//! Contains the background tasks that poll OpenCode sessions for new messages
//! and relay them to Slack threads using streaming with `task_update` chunks
//! for tool progress.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, Notify};
use tracing::{debug, error, info};

use crate::blockkit::{markdown_to_blocks, split_around_tables, MdTextSegment};

use super::api::{
    append_stream, chunk_for_slack, post_message, post_message_with_blocks, start_stream,
    stop_stream,
};
use super::auth::{SlackAuth, SlackSessionMap};
use super::formatting::{convert_markdown_tables, markdown_to_slack_mrkdwn};
use super::state::{slack_thread_link, SlackState};
use super::tools::{build_task_chunks, fetch_session_messages_with_tools};
use super::types::SlackLogLevel;

/// Build the `attachments` array for Slack from table blocks.
///
/// Slack’s `table` block must go in `attachments`, not top-level `blocks`.
/// Only one table per message is allowed; extras are dropped.
/// Returns `None` if there are no table blocks.
fn table_attachments(table_blocks: &[serde_json::Value]) -> Option<Vec<serde_json::Value>> {
    if table_blocks.is_empty() {
        return None;
    }
    // Slack allows only one table per message.
    Some(vec![serde_json::json!({ "blocks": [&table_blocks[0]] })])
}
// ── Response Batching ───────────────────────────────────────────────────

/// Spawn a background task that periodically checks for pending response batches
/// and sends them to Slack.
pub async fn spawn_response_batcher(
    auth: SlackAuth,
    state: Arc<Mutex<SlackState>>,
    batch_interval_secs: u64,
    _event_tx: tokio::sync::mpsc::UnboundedSender<super::types::SlackBackgroundEvent>,
) {
    let client = reqwest::Client::new();
    let mut interval = tokio::time::interval(Duration::from_secs(batch_interval_secs));

    loop {
        interval.tick().await;

        // Collect pending batches.
        let batches: Vec<(String, String, String)> = {
            let mut st = state.lock().await;
            let mut out = Vec::new();

            let session_ids: Vec<String> = st.response_buffers.keys().cloned().collect();
            for session_id in session_ids {
                if let Some(text) = st.response_buffers.remove(&session_id) {
                    if !text.is_empty() {
                        if let Some((channel, thread_ts)) = st.session_threads.get(&session_id) {
                            out.push((channel.clone(), thread_ts.clone(), text));
                        }
                    }
                }
            }

            out
        };

        // Send each batch to Slack.
        let batch_count = batches.len();
        for (channel, thread_ts, text) in batches {
            // Try to send as Block Kit blocks for rich formatting.
            let bk = markdown_to_blocks(&text);
            let fallback = markdown_to_slack_mrkdwn(&text);
            let table_att = table_attachments(&bk.table_blocks);
            let att_ref = table_att.as_deref();
            if !bk.blocks.is_empty() || table_att.is_some() {
                match post_message_with_blocks(
                    &client,
                    &auth.bot_token,
                    &channel,
                    &fallback,
                    &bk.blocks,
                    att_ref,
                    Some(&thread_ts),
                )
                .await
                {
                    Ok(_) => debug!(
                        "Relayed response batch with blocks to Slack thread {}",
                        thread_ts
                    ),
                    Err(e) => error!("Failed to relay blocks to Slack: {}", e),
                }
            } else {
                let chunks = chunk_for_slack(&fallback, 39_000);
                for chunk in chunks {
                    match post_message(&client, &auth.bot_token, &channel, &chunk, Some(&thread_ts))
                        .await
                    {
                        Ok(_) => debug!("Relayed response batch to Slack thread {}", thread_ts),
                        Err(e) => error!("Failed to relay to Slack: {}", e),
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }

        // Update metrics for batches sent via the batcher.
        if batch_count > 0 {
            let mut st = state.lock().await;
            st.metrics.batches_sent += batch_count as u64;
            st.log(
                SlackLogLevel::Info,
                format!("Batcher sent {} response batch(es)", batch_count),
            );
        }
    }
}

// ── Live Relay Watcher ──────────────────────────────────────────────────

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
        // When notified by an SSE event we still add a small debounce so we
        // don't hammer the API while messages are streaming in rapidly.
        let debounce = Duration::from_millis(800);
        let mut idle_polls: u32 = 0;
        const IDLE_STOP_THRESHOLD: u32 = 3;
        let mut last_streamed_role: Option<String> = None;
        let mut label_emitted = false;
        // Track when the current stream was started so we can rotate it
        // periodically.  This prevents Slack from showing a "typing..."
        // indicator indefinitely for long-running sessions.
        let mut stream_started_at: Option<tokio::time::Instant> = None;
        // Maximum age of a single Slack stream before we finalize it and
        // start a fresh one on the next content poll.  60 seconds keeps
        // individual streams short-lived while still providing a smooth UX.
        const STREAM_MAX_AGE: Duration = Duration::from_secs(60);

        // Accumulated raw markdown text for the current stream.  When a
        // stream is stopped we convert this to Block Kit blocks and use
        // `chat.update` to replace the streamed message with rich formatting.
        let mut accumulated_stream_md = String::new();
        // Separate accumulator for raw (unconverted) markdown.  This is
        // used for Block Kit conversion at finalization, because
        // `convert_markdown_tables` wraps tables in code fences that hide
        // the pipe syntax from the Block Kit parser.
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
                    // Debounce: wait a short time so rapid-fire events coalesce.
                    tokio::time::sleep(debounce).await;
                }
            }

            // ── External stop request (from SseSessionIdle) ─────────
            // Check whether an external handler has asked us to finalize
            // the active stream.  We handle it here (inside the watcher
            // loop) so the stream lifecycle is never modified by two
            // concurrent tasks.
            {
                let mut s = slack_state.lock().await;
                if s.stream_stop_requested.remove(&session_id) {
                    if let Some(ts) = s.streaming_messages.remove(&session_id) {
                        drop(s); // release lock before async call
                        tracing::info!(
                            "Slack relay: honoring external stop request for stream {} session {}",
                            ts,
                            &session_id[..8.min(session_id.len())]
                        );
                        if let Err(e) = stop_stream(&client, &bot_token, &channel, &ts).await {
                            tracing::warn!(
                                "Slack relay: stopStream (external request) failed: {}",
                                e
                            );
                        }
                        // Clear accumulators — the streamed markdown is the final content.
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
                // If the session has been idle long enough and we have an active
                // stream, finalize it so the Slack message stops "typing".
                if idle_polls >= IDLE_STOP_THRESHOLD {
                    let stream_ts = {
                        let s = slack_state.lock().await;
                        s.streaming_messages.get(&session_id).cloned()
                    };
                    if let Some(ref ts) = stream_ts {
                        tracing::info!(
                            "Slack relay: stopping stream {} for session {} (idle for {} polls)",
                            ts,
                            &session_id[..8.min(session_id.len())],
                            idle_polls
                        );
                        if let Err(e) = stop_stream(&client, &bot_token, &channel, ts).await {
                            tracing::warn!("Slack relay: stopStream failed: {}", e);
                        }
                        // Clear accumulators — the streamed markdown is the final content.
                        accumulated_stream_md.clear();
                        accumulated_raw_md.clear();
                        let mut s = slack_state.lock().await;
                        s.streaming_messages.remove(&session_id);
                        stream_started_at = None;
                    }
                }
                continue;
            }

            // We have new messages -- reset idle counter.
            idle_polls = 0;

            // Collect new messages.
            let new_messages: Vec<_> = messages
                .into_iter()
                .skip(offset)
                .filter(|m| !m.text.is_empty() || !m.tools.is_empty())
                .collect();

            if new_messages.is_empty() {
                // Offset still advances even if all were empty.
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

            // Separate text content and tool task_update chunks.
            let mut all_task_chunks: Vec<serde_json::Value> = Vec::new();
            let mut groups: Vec<(String, Vec<String>)> = Vec::new();
            // Parallel groups storing the raw (unconverted) text for Block Kit.
            let mut groups_raw: Vec<(String, Vec<String>)> = Vec::new();
            // Collect subagent thread links for this parent session so we can
            // replace "task" tool outputs with links to the child threads.
            let subagent_links: Vec<String> = {
                let s = slack_state.lock().await;
                s.subagent_threads
                    .iter()
                    .filter(|(_child_sid, (_ch, _ts, parent_sid))| parent_sid == &session_id)
                    .map(|(_child_sid, (ch, ts, _parent_sid))| slack_thread_link(ch, ts))
                    .collect()
            };

            for msg in &new_messages {
                // Collect task_update chunks from tool parts.
                if !msg.tools.is_empty() {
                    let mut chunks = build_task_chunks(&msg.tools);
                    // For "task" tool calls, replace the output with a link to
                    // the subagent's dedicated thread (if one exists).
                    if !subagent_links.is_empty() {
                        let mut link_idx = 0;
                        for chunk in &mut chunks {
                            let is_task = chunk
                                .get("title")
                                .and_then(|t| t.as_str())
                                .map(|t| t.starts_with("`task`:"))
                                .unwrap_or(false);
                            if is_task {
                                let link = if link_idx < subagent_links.len() {
                                    &subagent_links[link_idx]
                                } else {
                                    subagent_links.last().unwrap()
                                };
                                chunk["output"] = serde_json::Value::String(format!(
                                    ":thread: Subagent thread: {}",
                                    link
                                ));
                                link_idx += 1;
                            }
                        }
                    }
                    all_task_chunks.extend(chunks);
                }

                // Group text by role for the markdown portion.
                if !msg.text.is_empty() {
                    let converted = convert_markdown_tables(&msg.text);
                    let raw = msg.text.clone();
                    if let Some(last) = groups.last_mut() {
                        if last.0 == msg.role {
                            last.1.push(converted);
                            // Keep groups_raw in sync.
                            if let Some(last_raw) = groups_raw.last_mut() {
                                last_raw.1.push(raw);
                            }
                            continue;
                        }
                    }
                    groups.push((msg.role.clone(), vec![converted]));
                    groups_raw.push((msg.role.clone(), vec![raw]));
                }
            }

            let mut markdown_parts: Vec<String> = Vec::new();
            for (i, (role, texts)) in groups.iter().enumerate() {
                let body = texts.join("\n");
                // Skip divider + header for the first group if it continues
                // the same role from the previous poll cycle.
                let same_as_last = i == 0 && last_streamed_role.as_deref() == Some(role.as_str());

                // Format body based on role.
                let formatted = if role == "user" {
                    // Blockquote each line for user messages.
                    body.lines()
                        .map(|l| format!("> {}", l))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else if role == "error" {
                    // Prefix error messages with :x: emoji.
                    format!(":x: {}", body)
                } else {
                    body
                };

                if same_as_last {
                    markdown_parts.push(formatted);
                } else {
                    markdown_parts.push(format!("\n{}", formatted));
                }
            }
            let mut relay_text = markdown_parts.join("\n");

            // Build the raw (unconverted) relay text for Block Kit accumulation.
            let relay_text_raw = {
                let mut raw_parts: Vec<String> = Vec::new();
                for (i, (role, texts)) in groups_raw.iter().enumerate() {
                    let body = texts.join("\n");
                    let same_as_last =
                        i == 0 && last_streamed_role.as_deref() == Some(role.as_str());

                    // Format body based on role.
                    let formatted = if role == "user" {
                        body.lines()
                            .map(|l| format!("> {}", l))
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else if role == "error" {
                        format!(":x: {}", body)
                    } else {
                        body
                    };

                    if same_as_last {
                        raw_parts.push(formatted);
                    } else {
                        raw_parts.push(format!("\n{}", formatted));
                    }
                }
                let mut raw = raw_parts.join("\n");
                // Prepend label if this is the first relay.
                if !label_emitted {
                    if let Some(ref lbl) = label {
                        raw = format!("{}\n{}", lbl, raw);
                    }
                }
                raw
            };

            // Diagnostic: compare converted vs raw relay text.
            if relay_text != relay_text_raw {
                tracing::info!(
                    "Slack relay: relay_text DIFFERS from relay_text_raw — converted_len={}, raw_len={}, raw_preview={:?}",
                    relay_text.len(),
                    relay_text_raw.len(),
                    &relay_text_raw[..relay_text_raw.len().min(400)]
                );
            } else {
                tracing::info!(
                    "Slack relay: relay_text SAME as relay_text_raw — len={}",
                    relay_text.len()
                );
            }

            // Prepend optional label on first relay (e.g. subagent indicator).
            if !label_emitted {
                if let Some(ref lbl) = label {
                    relay_text = format!("{}\n{}", lbl, relay_text);
                }
                label_emitted = true;
            }
            // Update last_streamed_role to the final group's role.
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
            // If the current stream has been alive longer than STREAM_MAX_AGE,
            // finalize it now and let the code below start a fresh one.
            // This prevents Slack from showing a "typing..." indicator
            // indefinitely for long-running sessions.
            //
            // Exception: if the latest message contains a tool call that is
            // still running/pending, the session is in the middle of a
            // long-running tool (e.g. bash, task) — skip rotation so we don't
            // break the live task_update timeline mid-execution.
            if let Some(ref stream_ts) = active_stream_ts {
                if let Some(started) = stream_started_at {
                    if started.elapsed() >= STREAM_MAX_AGE {
                        // Check whether any tool in the latest new messages is
                        // still in-flight (running / pending / partial-call).
                        let has_inflight_tool = new_messages.last().map_or(false, |m| {
                            m.tools.iter().any(|t| {
                                matches!(
                                    t.status.as_str(),
                                    "running" | "pending" | "call" | "partial-call"
                                )
                            })
                        });

                        if has_inflight_tool {
                            tracing::debug!(
                                "Slack relay: stream {} for session {} is {}s old but has in-flight tools, skipping rotation",
                                stream_ts,
                                &session_id[..8.min(session_id.len())],
                                started.elapsed().as_secs()
                            );
                        } else {
                            tracing::info!(
                                "Slack relay: rotating stream {} for session {} (age {}s > {}s max)",
                                stream_ts,
                                &session_id[..8.min(session_id.len())],
                                started.elapsed().as_secs(),
                                STREAM_MAX_AGE.as_secs()
                            );
                            if let Err(e) =
                                stop_stream(&client, &bot_token, &channel, stream_ts).await
                            {
                                tracing::warn!("Slack relay: stopStream (rotation) failed: {}", e);
                            }
                            // Clear accumulators — the streamed markdown is the final content.
                            accumulated_stream_md.clear();
                            accumulated_raw_md.clear();
                            {
                                let mut s = slack_state.lock().await;
                                s.streaming_messages.remove(&session_id);
                            }
                            active_stream_ts = None;
                            stream_started_at = None;
                            // Diagnostic: log what relay_text will go into the new stream.
                            tracing::info!(
                                "Slack relay: rotation complete, relay_text for new stream: len={}, offset={}, total_count={}, preview={:?}",
                                relay_text.len(),
                                offset,
                                total_count,
                                &relay_text[..relay_text.len().min(300)]
                            );
                        }
                    }
                }
            }

            // ── Table-aware segment relay ──────────────────────────
            // Check whether the raw relay text contains any markdown
            // tables.  If so, split it into alternating Text / Table
            // segments and handle each one:
            //   • Text  → stream/append as normal
            //   • Table → finalize the active stream, post the table
            //             as a native Block Kit table message, then
            //             start a fresh stream for subsequent text.
            // If there are no tables, fall through to the original
            // stream/append logic unchanged.
            let raw_segments = split_around_tables(&relay_text_raw);
            let has_table = raw_segments
                .iter()
                .any(|s| matches!(s, MdTextSegment::Table(_)));

            if has_table {
                tracing::info!(
                    "Slack relay: relay_text_raw contains {} segment(s) with table(s), using segmented relay for session {}",
                    raw_segments.len(),
                    &session_id[..8.min(session_id.len())]
                );

                // We also split the *converted* relay_text so we can
                // stream the mrkdwn-converted text for text segments.
                // Note: convert_markdown_tables wraps tables in code
                // fences, but split_around_tables detects the raw
                // pipe-delimited tables.  For the converted text we
                // just use it as the stream content for text segments
                // (tables in the converted text become code fences
                // which is fine as fallback).
                let mut task_chunks_sent = false;

                for segment in &raw_segments {
                    match segment {
                        MdTextSegment::Text(text_raw) => {
                            // Convert for streaming display.
                            let text_converted = convert_markdown_tables(text_raw);
                            let text_mrkdwn = markdown_to_slack_mrkdwn(text_raw);

                            if text_raw.trim().is_empty() {
                                continue;
                            }

                            // Check if we have an active stream.
                            let current_stream = {
                                let s = slack_state.lock().await;
                                s.streaming_messages.get(&session_id).cloned()
                            };

                            if let Some(ref stream_ts) = current_stream {
                                // Append text to the existing stream.
                                let text_chunks = chunk_for_slack(&text_converted, 11_000);
                                accumulated_stream_md.push_str(&text_converted);
                                accumulated_raw_md.push_str(text_raw);
                                for (ci, chunk) in text_chunks.iter().enumerate() {
                                    let chunks_for_append =
                                        if ci == 0 && has_task_chunks && !task_chunks_sent {
                                            task_chunks_sent = true;
                                            Some(all_task_chunks.as_slice())
                                        } else {
                                            None
                                        };
                                    if let Err(e) = append_stream(
                                        &client,
                                        &bot_token,
                                        &channel,
                                        stream_ts,
                                        chunk,
                                        chunks_for_append,
                                    )
                                    .await
                                    {
                                        tracing::warn!(
                                            "Slack relay: appendStream (segmented text) failed: {}",
                                            e
                                        );
                                        // Fall back to posting.
                                        let _ = post_message(
                                            &client,
                                            &bot_token,
                                            &channel,
                                            &text_mrkdwn,
                                            Some(&thread_ts),
                                        )
                                        .await;
                                        let mut s = slack_state.lock().await;
                                        s.streaming_messages.remove(&session_id);
                                        stream_started_at = None;
                                        accumulated_stream_md.clear();
                                        accumulated_raw_md.clear();
                                        break;
                                    }
                                }
                            } else {
                                // Start a new stream for this text segment.
                                let empty_chunks: Vec<serde_json::Value> = vec![];
                                let initial_chunks: &[serde_json::Value] =
                                    if has_task_chunks && !task_chunks_sent {
                                        task_chunks_sent = true;
                                        &all_task_chunks
                                    } else {
                                        &empty_chunks
                                    };
                                match start_stream(
                                    &client,
                                    &bot_token,
                                    &channel,
                                    &thread_ts,
                                    Some(&text_converted),
                                    Some(initial_chunks),
                                    Some("timeline"),
                                )
                                .await
                                {
                                    Ok(new_ts) => {
                                        tracing::info!(
                                            "Slack relay: started segment stream {} for session {}",
                                            new_ts,
                                            &session_id[..8.min(session_id.len())]
                                        );
                                        let mut s = slack_state.lock().await;
                                        s.streaming_messages.insert(session_id.clone(), new_ts);
                                        stream_started_at = Some(tokio::time::Instant::now());
                                        accumulated_stream_md = text_converted.clone();
                                        accumulated_raw_md = text_raw.clone();
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Slack relay: startStream (segmented text) failed: {}",
                                            e
                                        );
                                        // Fall back to posting.
                                        let bk = markdown_to_blocks(text_raw);
                                        let fallback = markdown_to_slack_mrkdwn(text_raw);
                                        let table_att = table_attachments(&bk.table_blocks);
                                        let att_ref = table_att.as_deref();
                                        if !bk.blocks.is_empty() || table_att.is_some() {
                                            let _ = post_message_with_blocks(
                                                &client,
                                                &bot_token,
                                                &channel,
                                                &fallback,
                                                &bk.blocks,
                                                att_ref,
                                                Some(&thread_ts),
                                            )
                                            .await;
                                        } else {
                                            let _ = post_message(
                                                &client,
                                                &bot_token,
                                                &channel,
                                                &fallback,
                                                Some(&thread_ts),
                                            )
                                            .await;
                                        }
                                    }
                                }
                            }
                        }
                        MdTextSegment::Table(table_raw) => {
                            // ── Finalize active stream before posting table ──
                            let current_stream = {
                                let s = slack_state.lock().await;
                                s.streaming_messages.get(&session_id).cloned()
                            };
                            if let Some(ref stream_ts) = current_stream {
                                tracing::info!(
                                    "Slack relay: finalizing stream {} before posting table for session {}",
                                    stream_ts, &session_id[..8.min(session_id.len())]
                                );
                                if let Err(e) =
                                    stop_stream(&client, &bot_token, &channel, stream_ts).await
                                {
                                    tracing::warn!(
                                        "Slack relay: stopStream (pre-table) failed: {}",
                                        e
                                    );
                                }
                                // Clear accumulators — the streamed markdown is the final content.
                                accumulated_stream_md.clear();
                                accumulated_raw_md.clear();
                                {
                                    let mut s = slack_state.lock().await;
                                    s.streaming_messages.remove(&session_id);
                                }
                                stream_started_at = None;
                            }

                            // ── Post the table as a native Block Kit message ──
                            let bk = markdown_to_blocks(table_raw);
                            let tbl_att = table_attachments(&bk.table_blocks);
                            let att_ref = tbl_att.as_deref();
                            // Use the table's mrkdwn-formatted version as fallback text.
                            let fallback = markdown_to_slack_mrkdwn(table_raw);

                            // Combine any non-table blocks (unlikely, but safe)
                            // with the table attachment.
                            if !bk.blocks.is_empty() || tbl_att.is_some() {
                                match post_message_with_blocks(
                                    &client,
                                    &bot_token,
                                    &channel,
                                    &fallback,
                                    &bk.blocks,
                                    att_ref,
                                    Some(&thread_ts),
                                )
                                .await
                                {
                                    Ok(_ts) => {
                                        tracing::info!(
                                            "Slack relay: posted native table block for session {}",
                                            &session_id[..8.min(session_id.len())]
                                        );
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Slack relay: failed to post table block: {}",
                                            e
                                        );
                                        // Fall back to plain message with code-fenced table.
                                        let _ = post_message(
                                            &client,
                                            &bot_token,
                                            &channel,
                                            &fallback,
                                            Some(&thread_ts),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                // markdown_to_blocks didn't produce a table
                                // block — post as code-fenced fallback.
                                let _ = post_message(
                                    &client,
                                    &bot_token,
                                    &channel,
                                    &fallback,
                                    Some(&thread_ts),
                                )
                                .await;
                            }
                        }
                    }
                }
            } else {
                // ── Original stream/append logic (no tables) ────────
                if let Some(ref stream_ts) = active_stream_ts {
                    // Append to the existing stream with both text and task chunks.
                    // appendStream has a 12k char limit for text; chunk if needed.
                    let text_chunks = chunk_for_slack(&relay_text, 11_000);
                    // Accumulate the full relay text for Block Kit conversion
                    // when the stream is eventually stopped.
                    accumulated_stream_md.push_str(&relay_text);
                    accumulated_raw_md.push_str(&relay_text_raw);
                    for (i, chunk) in text_chunks.iter().enumerate() {
                        // Attach task chunks only on the first text chunk to avoid
                        // duplicate task_update entries.
                        let chunks_for_append = if i == 0 && has_task_chunks {
                            Some(all_task_chunks.as_slice())
                        } else {
                            None
                        };
                        if let Err(e) = append_stream(
                            &client,
                            &bot_token,
                            &channel,
                            stream_ts,
                            chunk,
                            chunks_for_append,
                        )
                        .await
                        {
                            let err_str = format!("{}", e);
                            tracing::warn!("Slack relay: appendStream failed ({})", err_str,);

                            if err_str.contains("stream_mode_mismatch") {
                                // The stream was started without task_display_mode
                                // but we're now trying to append task_update chunks.
                                // Fix: stop the old stream and start a fresh one
                                // with proper task mode.
                                tracing::info!(
                                    "Slack relay: restarting stream with task_display_mode for session {}",
                                    &session_id[..8.min(session_id.len())]
                                );
                                let _ = stop_stream(&client, &bot_token, &channel, stream_ts).await;
                                // Clear accumulators — the streamed markdown is the final content.
                                accumulated_stream_md.clear();
                                accumulated_raw_md.clear();
                                {
                                    let mut s = slack_state.lock().await;
                                    s.streaming_messages.remove(&session_id);
                                }
                                stream_started_at = None;

                                // Gather remaining text chunks (current + rest).
                                let remaining_text = text_chunks[i..].join("");
                                let empty_chunks: Vec<serde_json::Value> = vec![];
                                let restart_chunks: &[serde_json::Value] = if has_task_chunks {
                                    &all_task_chunks
                                } else {
                                    &empty_chunks
                                };
                                match start_stream(
                                    &client,
                                    &bot_token,
                                    &channel,
                                    &thread_ts,
                                    Some(&remaining_text),
                                    Some(restart_chunks),
                                    Some("timeline"),
                                )
                                .await
                                {
                                    Ok(new_ts) => {
                                        tracing::info!(
                                            "Slack relay: restarted stream {} for session {}",
                                            new_ts,
                                            &session_id[..8.min(session_id.len())]
                                        );
                                        let mut s = slack_state.lock().await;
                                        s.streaming_messages.insert(session_id.clone(), new_ts);
                                        stream_started_at = Some(tokio::time::Instant::now());
                                        accumulated_stream_md = remaining_text.clone();
                                        accumulated_raw_md = relay_text_raw.clone();
                                    }
                                    Err(e2) => {
                                        tracing::warn!(
                                            "Slack relay: restart startStream also failed ({}), falling back to post",
                                            e2
                                        );
                                        let bk = markdown_to_blocks(&relay_text_raw);
                                        let fallback = markdown_to_slack_mrkdwn(&remaining_text);
                                        let table_att = table_attachments(&bk.table_blocks);
                                        let att_ref = table_att.as_deref();
                                        if !bk.blocks.is_empty() || table_att.is_some() {
                                            let _ = post_message_with_blocks(
                                                &client,
                                                &bot_token,
                                                &channel,
                                                &fallback,
                                                &bk.blocks,
                                                att_ref,
                                                Some(&thread_ts),
                                            )
                                            .await;
                                        } else {
                                            let _ = post_message(
                                                &client,
                                                &bot_token,
                                                &channel,
                                                &fallback,
                                                Some(&thread_ts),
                                            )
                                            .await;
                                        }
                                    }
                                }
                            } else {
                                // Non-mode-mismatch error: stream may have been
                                // stopped externally; fall back to post.
                                let formatted = markdown_to_slack_mrkdwn(chunk);
                                let _ = post_message(
                                    &client,
                                    &bot_token,
                                    &channel,
                                    &formatted,
                                    Some(&thread_ts),
                                )
                                .await;
                                // Clear the stale stream reference.
                                let mut s = slack_state.lock().await;
                                s.streaming_messages.remove(&session_id);
                                stream_started_at = None;
                                accumulated_stream_md.clear();
                                accumulated_raw_md.clear();
                            }
                            break;
                        }
                    }
                } else {
                    // Always start with timeline display mode so that task_update
                    // chunks can be appended later without a mode mismatch.
                    // IMPORTANT: pass an empty chunks array (not None) even when
                    // there are no task chunks yet — Slack only registers the
                    // stream as task-capable when the `chunks` field is present
                    // in the startStream call alongside `task_display_mode`.
                    let empty_chunks: Vec<serde_json::Value> = vec![];
                    let initial_chunks: &[serde_json::Value] = if has_task_chunks {
                        &all_task_chunks
                    } else {
                        &empty_chunks
                    };
                    let display_mode = Some("timeline");
                    match start_stream(
                        &client,
                        &bot_token,
                        &channel,
                        &thread_ts,
                        Some(&relay_text),
                        Some(initial_chunks),
                        display_mode,
                    )
                    .await
                    {
                        Ok(stream_ts) => {
                            tracing::info!(
                                "Slack relay: started NEW stream {} for session {} (task_chunks={}, relay_text_len={}, offset={}, total_count={}, preview={:?})",
                                stream_ts,
                                &session_id[..8.min(session_id.len())],
                                all_task_chunks.len(),
                                relay_text.len(),
                                offset,
                                total_count,
                                &relay_text[..relay_text.len().min(200)]
                            );
                            let mut s = slack_state.lock().await;
                            s.streaming_messages.insert(session_id.clone(), stream_ts);
                            stream_started_at = Some(tokio::time::Instant::now());
                            accumulated_stream_md = relay_text.clone();
                            accumulated_raw_md = relay_text_raw.clone();
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Slack relay: startStream failed ({}), falling back to post",
                                e
                            );
                            // Fall back to regular posting with Block Kit blocks.
                            let bk = markdown_to_blocks(&relay_text_raw);
                            let fallback = markdown_to_slack_mrkdwn(&relay_text);
                            let table_att = table_attachments(&bk.table_blocks);
                            let att_ref = table_att.as_deref();
                            if !bk.blocks.is_empty() || table_att.is_some() {
                                let _ = post_message_with_blocks(
                                    &client,
                                    &bot_token,
                                    &channel,
                                    &fallback,
                                    &bk.blocks,
                                    att_ref,
                                    Some(&thread_ts),
                                )
                                .await;
                            } else {
                                let post_chunks = chunk_for_slack(&fallback, 39_000);
                                for chunk in &post_chunks {
                                    let _ = post_message(
                                        &client,
                                        &bot_token,
                                        &channel,
                                        chunk,
                                        Some(&thread_ts),
                                    )
                                    .await;
                                }
                            }
                        }
                    }
                }
            }

            // Update offset and persist the session map.
            {
                let mut s = slack_state.lock().await;
                s.session_msg_offset.insert(session_id.clone(), total_count);
                // Persist to disk so we survive restarts.
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
