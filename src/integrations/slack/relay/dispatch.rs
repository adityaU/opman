//! Relay dispatch – routes relay text to the appropriate handler based on
//! whether the content contains tables and whether a stream is already active.

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::blockkit::split_around_tables;

use super::super::state::SlackState;
use super::segments::relay_segments;
use super::streaming::{append_to_stream, start_new_stream};

/// Dispatch the relay text to the appropriate handler: segmented (tables),
/// append (existing stream), or start-new-stream.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn dispatch_relay(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    relay_text: &str,
    relay_text_raw: &str,
    all_task_chunks: &[serde_json::Value],
    has_task_chunks: bool,
    active_stream_ts: Option<&str>,
    offset: usize,
    total_count: usize,
    slack_state: &Arc<Mutex<SlackState>>,
    accumulated_stream_md: &mut String,
    accumulated_raw_md: &mut String,
    stream_started_at: &mut Option<tokio::time::Instant>,
) {
    let raw_segments = split_around_tables(relay_text_raw);
    let has_table = raw_segments
        .iter()
        .any(|s| matches!(s, crate::blockkit::MdTextSegment::Table(_)));

    if has_table {
        tracing::info!(
            "Slack relay: relay_text_raw contains {} segment(s) with table(s), using segmented relay for session {}",
            raw_segments.len(),
            &session_id[..8.min(session_id.len())]
        );
        relay_segments(
            client,
            bot_token,
            channel,
            thread_ts,
            session_id,
            &raw_segments,
            all_task_chunks,
            has_task_chunks,
            slack_state,
            accumulated_stream_md,
            accumulated_raw_md,
            stream_started_at,
        )
        .await;
    } else if let Some(stream_ts) = active_stream_ts {
        append_to_stream(
            client,
            bot_token,
            channel,
            thread_ts,
            session_id,
            stream_ts,
            relay_text,
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
        start_new_stream(
            client,
            bot_token,
            channel,
            thread_ts,
            session_id,
            relay_text,
            relay_text_raw,
            all_task_chunks,
            has_task_chunks,
            offset,
            total_count,
            slack_state,
            accumulated_stream_md,
            accumulated_raw_md,
            stream_started_at,
        )
        .await;
    }
}
