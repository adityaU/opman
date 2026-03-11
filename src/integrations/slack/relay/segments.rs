//! Table-aware segment relay – splits relay text around markdown tables and
//! posts each segment appropriately (stream for text, native Block Kit for
//! tables).

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::blockkit::{markdown_to_blocks, MdTextSegment};

use super::super::api::{
    append_stream, chunk_for_slack, post_message, post_message_with_blocks, start_stream,
    stop_stream,
};
use super::super::formatting::{convert_markdown_tables, markdown_to_slack_mrkdwn};
use super::super::state::SlackState;
use super::table_attachments;

/// Relay a set of raw markdown segments (produced by `split_around_tables`) to
/// Slack.  Text segments are streamed; table segments are posted as native
/// Block Kit table messages.
///
/// Returns the updated `(accumulated_stream_md, accumulated_raw_md,
/// stream_started_at)` state.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn relay_segments(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    raw_segments: &[MdTextSegment],
    all_task_chunks: &[serde_json::Value],
    has_task_chunks: bool,
    slack_state: &Arc<Mutex<SlackState>>,
    accumulated_stream_md: &mut String,
    accumulated_raw_md: &mut String,
    stream_started_at: &mut Option<tokio::time::Instant>,
) {
    let mut task_chunks_sent = false;

    for segment in raw_segments {
        match segment {
            MdTextSegment::Text(text_raw) => {
                relay_text_segment(
                    client,
                    bot_token,
                    channel,
                    thread_ts,
                    session_id,
                    text_raw,
                    all_task_chunks,
                    has_task_chunks,
                    &mut task_chunks_sent,
                    slack_state,
                    accumulated_stream_md,
                    accumulated_raw_md,
                    stream_started_at,
                )
                .await;
            }
            MdTextSegment::Table(table_raw) => {
                relay_table_segment(
                    client,
                    bot_token,
                    channel,
                    thread_ts,
                    session_id,
                    table_raw,
                    slack_state,
                    accumulated_stream_md,
                    accumulated_raw_md,
                    stream_started_at,
                )
                .await;
            }
        }
    }
}

/// Relay a text segment: append to an existing stream or start a new one.
#[allow(clippy::too_many_arguments)]
async fn relay_text_segment(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    text_raw: &str,
    all_task_chunks: &[serde_json::Value],
    has_task_chunks: bool,
    task_chunks_sent: &mut bool,
    slack_state: &Arc<Mutex<SlackState>>,
    accumulated_stream_md: &mut String,
    accumulated_raw_md: &mut String,
    stream_started_at: &mut Option<tokio::time::Instant>,
) {
    let text_converted = convert_markdown_tables(text_raw);
    let text_mrkdwn = markdown_to_slack_mrkdwn(text_raw);

    if text_raw.trim().is_empty() {
        return;
    }

    let current_stream = {
        let s = slack_state.lock().await;
        s.streaming_messages.get(session_id).cloned()
    };

    if let Some(ref stream_ts) = current_stream {
        // Append text to the existing stream.
        let text_chunks = chunk_for_slack(&text_converted, 11_000);
        accumulated_stream_md.push_str(&text_converted);
        accumulated_raw_md.push_str(text_raw);
        for (ci, chunk) in text_chunks.iter().enumerate() {
            let chunks_for_append = if ci == 0 && has_task_chunks && !*task_chunks_sent {
                *task_chunks_sent = true;
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
                tracing::warn!("Slack relay: appendStream (segmented text) failed: {}", e);
                // Fall back to posting.
                let _ =
                    post_message(client, bot_token, channel, &text_mrkdwn, Some(thread_ts)).await;
                let mut s = slack_state.lock().await;
                s.streaming_messages.remove(session_id);
                *stream_started_at = None;
                accumulated_stream_md.clear();
                accumulated_raw_md.clear();
                break;
            }
        }
    } else {
        // Start a new stream for this text segment.
        let empty_chunks: Vec<serde_json::Value> = vec![];
        let initial_chunks: &[serde_json::Value] = if has_task_chunks && !*task_chunks_sent {
            *task_chunks_sent = true;
            all_task_chunks
        } else {
            &empty_chunks
        };
        match start_stream(
            client,
            bot_token,
            channel,
            thread_ts,
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
                s.streaming_messages.insert(session_id.to_owned(), new_ts);
                *stream_started_at = Some(tokio::time::Instant::now());
                *accumulated_stream_md = text_converted.clone();
                *accumulated_raw_md = text_raw.to_owned();
            }
            Err(e) => {
                tracing::warn!("Slack relay: startStream (segmented text) failed: {}", e);
                // Fall back to posting.
                let bk = markdown_to_blocks(text_raw);
                let fallback = markdown_to_slack_mrkdwn(text_raw);
                let table_att = table_attachments(&bk.table_blocks);
                let att_ref = table_att.as_deref();
                if !bk.blocks.is_empty() || table_att.is_some() {
                    let _ = post_message_with_blocks(
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
                    let _ =
                        post_message(client, bot_token, channel, &fallback, Some(thread_ts)).await;
                }
            }
        }
    }
}

/// Relay a table segment: finalize any active stream, then post the table as
/// a native Block Kit table message.
#[allow(clippy::too_many_arguments)]
async fn relay_table_segment(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    table_raw: &str,
    slack_state: &Arc<Mutex<SlackState>>,
    accumulated_stream_md: &mut String,
    accumulated_raw_md: &mut String,
    stream_started_at: &mut Option<tokio::time::Instant>,
) {
    // ── Finalize active stream before posting table ──
    let current_stream = {
        let s = slack_state.lock().await;
        s.streaming_messages.get(session_id).cloned()
    };
    if let Some(ref stream_ts) = current_stream {
        tracing::info!(
            "Slack relay: finalizing stream {} before posting table for session {}",
            stream_ts,
            &session_id[..8.min(session_id.len())]
        );
        if let Err(e) = stop_stream(client, bot_token, channel, stream_ts).await {
            tracing::warn!("Slack relay: stopStream (pre-table) failed: {}", e);
        }
        accumulated_stream_md.clear();
        accumulated_raw_md.clear();
        {
            let mut s = slack_state.lock().await;
            s.streaming_messages.remove(session_id);
        }
        *stream_started_at = None;
    }

    // ── Post the table as a native Block Kit message ──
    let bk = markdown_to_blocks(table_raw);
    let tbl_att = table_attachments(&bk.table_blocks);
    let att_ref = tbl_att.as_deref();
    let fallback = markdown_to_slack_mrkdwn(table_raw);

    if !bk.blocks.is_empty() || tbl_att.is_some() {
        match post_message_with_blocks(
            client, bot_token, channel, &fallback, &bk.blocks, att_ref, Some(thread_ts),
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
                tracing::warn!("Slack relay: failed to post table block: {}", e);
                let _ =
                    post_message(client, bot_token, channel, &fallback, Some(thread_ts)).await;
            }
        }
    } else {
        let _ = post_message(client, bot_token, channel, &fallback, Some(thread_ts)).await;
    }
}
