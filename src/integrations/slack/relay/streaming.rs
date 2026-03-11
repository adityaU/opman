//! Stream/append relay logic – used when relay text contains no markdown tables.

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::blockkit::markdown_to_blocks;

use super::super::api::{
    append_stream, chunk_for_slack, post_message, start_stream, stop_stream,
};
use super::super::formatting::markdown_to_slack_mrkdwn;
use super::super::state::SlackState;
use super::post_blockkit_or_plain;
use super::table_attachments;

/// Append `relay_text` to an existing stream, handling `stream_mode_mismatch`
/// by restarting.
///
/// Returns the updated `(accumulated_stream_md, accumulated_raw_md,
/// stream_started_at)` state.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn append_to_stream(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    stream_ts: &str,
    relay_text: &str,
    relay_text_raw: &str,
    all_task_chunks: &[serde_json::Value],
    has_task_chunks: bool,
    slack_state: &Arc<Mutex<SlackState>>,
    accumulated_stream_md: &mut String,
    accumulated_raw_md: &mut String,
    stream_started_at: &mut Option<tokio::time::Instant>,
) {
    let text_chunks = chunk_for_slack(relay_text, 11_000);
    accumulated_stream_md.push_str(relay_text);
    accumulated_raw_md.push_str(relay_text_raw);
    for (i, chunk) in text_chunks.iter().enumerate() {
        let chunks_for_append = if i == 0 && has_task_chunks {
            Some(all_task_chunks)
        } else {
            None
        };
        if let Err(e) = append_stream(
            client,
            bot_token,
            channel,
            stream_ts,
            chunk,
            chunks_for_append,
        )
        .await
        {
            let err_str = format!("{}", e);
            tracing::warn!("Slack relay: appendStream failed ({})", err_str);

            if err_str.contains("stream_mode_mismatch") {
                handle_mode_mismatch_restart(
                    client,
                    bot_token,
                    channel,
                    thread_ts,
                    session_id,
                    stream_ts,
                    &text_chunks,
                    i,
                    relay_text_raw,
                    all_task_chunks,
                    has_task_chunks,
                    slack_state,
                    accumulated_stream_md,
                    accumulated_raw_md,
                    stream_started_at,
                )
                .await;
            } else {
                // Non-mode-mismatch error: stream may have been
                // stopped externally; fall back to post.
                let formatted = markdown_to_slack_mrkdwn(chunk);
                let _ =
                    post_message(client, bot_token, channel, &formatted, Some(thread_ts)).await;
                // Clear the stale stream reference.
                let mut s = slack_state.lock().await;
                s.streaming_messages.remove(session_id);
                *stream_started_at = None;
                accumulated_stream_md.clear();
                accumulated_raw_md.clear();
            }
            break;
        }
    }
}

/// Handle `stream_mode_mismatch` by stopping the old stream and starting a
/// fresh one with `task_display_mode`.
#[allow(clippy::too_many_arguments)]
async fn handle_mode_mismatch_restart(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    stream_ts: &str,
    text_chunks: &[String],
    chunk_idx: usize,
    relay_text_raw: &str,
    all_task_chunks: &[serde_json::Value],
    has_task_chunks: bool,
    slack_state: &Arc<Mutex<SlackState>>,
    accumulated_stream_md: &mut String,
    accumulated_raw_md: &mut String,
    stream_started_at: &mut Option<tokio::time::Instant>,
) {
    tracing::info!(
        "Slack relay: restarting stream with task_display_mode for session {}",
        &session_id[..8.min(session_id.len())]
    );
    let _ = stop_stream(client, bot_token, channel, stream_ts).await;
    accumulated_stream_md.clear();
    accumulated_raw_md.clear();
    {
        let mut s = slack_state.lock().await;
        s.streaming_messages.remove(session_id);
    }
    *stream_started_at = None;

    let remaining_text = text_chunks[chunk_idx..].join("");
    let empty_chunks: Vec<serde_json::Value> = vec![];
    let restart_chunks: &[serde_json::Value] = if has_task_chunks {
        all_task_chunks
    } else {
        &empty_chunks
    };
    match start_stream(
        client,
        bot_token,
        channel,
        thread_ts,
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
            s.streaming_messages.insert(session_id.to_owned(), new_ts);
            *stream_started_at = Some(tokio::time::Instant::now());
            *accumulated_stream_md = remaining_text.clone();
            *accumulated_raw_md = relay_text_raw.to_owned();
        }
        Err(e2) => {
            tracing::warn!(
                "Slack relay: restart startStream also failed ({}), falling back to post",
                e2
            );
            let remaining = markdown_to_slack_mrkdwn(&remaining_text);
            post_blockkit_or_plain(
                client,
                bot_token,
                channel,
                thread_ts,
                relay_text_raw,
                &remaining,
            )
            .await;
        }
    }
}

/// Start a brand-new stream for a session that has no active stream.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_new_stream(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    relay_text: &str,
    relay_text_raw: &str,
    all_task_chunks: &[serde_json::Value],
    has_task_chunks: bool,
    offset: usize,
    total_count: usize,
    slack_state: &Arc<Mutex<SlackState>>,
    accumulated_stream_md: &mut String,
    accumulated_raw_md: &mut String,
    stream_started_at: &mut Option<tokio::time::Instant>,
) {
    let empty_chunks: Vec<serde_json::Value> = vec![];
    let initial_chunks: &[serde_json::Value] = if has_task_chunks {
        all_task_chunks
    } else {
        &empty_chunks
    };
    let display_mode = Some("timeline");
    match start_stream(
        client,
        bot_token,
        channel,
        thread_ts,
        Some(relay_text),
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
            s.streaming_messages.insert(session_id.to_owned(), stream_ts);
            *stream_started_at = Some(tokio::time::Instant::now());
            *accumulated_stream_md = relay_text.to_owned();
            *accumulated_raw_md = relay_text_raw.to_owned();
        }
        Err(e) => {
            tracing::warn!(
                "Slack relay: startStream failed ({}), falling back to post",
                e
            );
            let fallback = markdown_to_slack_mrkdwn(relay_text);
            let bk = markdown_to_blocks(relay_text_raw);
            let table_att = table_attachments(&bk.table_blocks);
            let att_ref = table_att.as_deref();
            if !bk.blocks.is_empty() || table_att.is_some() {
                let _ = super::super::api::post_message_with_blocks(
                    client,
                    bot_token,
                    channel,
                    &fallback,
                    &bk.blocks,
                    att_ref,
                    Some(thread_ts),
                )
                .await;
            } else {
                let post_chunks = chunk_for_slack(&fallback, 39_000);
                for chunk in &post_chunks {
                    let _ = post_message(client, bot_token, channel, chunk, Some(thread_ts)).await;
                }
            }
        }
    }
}
