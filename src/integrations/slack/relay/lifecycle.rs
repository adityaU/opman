//! Stream lifecycle helpers – idle stop and stream rotation.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use super::super::api::stop_stream;
use super::super::state::SlackState;
use super::super::tools::StructuredMessage;

/// Maximum age of a single Slack stream before we finalize it and start a
/// fresh one on the next content poll.
const STREAM_MAX_AGE: Duration = Duration::from_secs(60);

/// Stop the active stream for a session that has been idle for too many
/// consecutive polls.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_idle_stop(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    session_id: &str,
    idle_polls: u32,
    slack_state: &Arc<Mutex<SlackState>>,
    accumulated_stream_md: &mut String,
    accumulated_raw_md: &mut String,
    stream_started_at: &mut Option<tokio::time::Instant>,
) {
    let stream_ts = {
        let s = slack_state.lock().await;
        s.streaming_messages.get(session_id).cloned()
    };
    if let Some(ref ts) = stream_ts {
        tracing::info!(
            "Slack relay: stopping stream {} for session {} (idle for {} polls)",
            ts,
            &session_id[..8.min(session_id.len())],
            idle_polls
        );
        if let Err(e) = stop_stream(client, bot_token, channel, ts).await {
            tracing::warn!("Slack relay: stopStream failed: {}", e);
        }
        accumulated_stream_md.clear();
        accumulated_raw_md.clear();
        let mut s = slack_state.lock().await;
        s.streaming_messages.remove(session_id);
        *stream_started_at = None;
    }
}

/// If the current stream has been alive longer than `STREAM_MAX_AGE`,
/// finalize it and clear the active stream reference so a fresh one is
/// started on the next content poll.
///
/// Skips rotation when the latest message contains an in-flight tool call
/// (running/pending/call/partial-call) to avoid breaking a live task_update
/// timeline mid-execution.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn maybe_rotate_stream(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    session_id: &str,
    new_messages: &[StructuredMessage],
    relay_text: &str,
    offset: usize,
    total_count: usize,
    slack_state: &Arc<Mutex<SlackState>>,
    active_stream_ts: &mut Option<String>,
    accumulated_stream_md: &mut String,
    accumulated_raw_md: &mut String,
    stream_started_at: &mut Option<tokio::time::Instant>,
) {
    if let Some(ref stream_ts) = active_stream_ts {
        if let Some(started) = *stream_started_at {
            if started.elapsed() >= STREAM_MAX_AGE {
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
                    if let Err(e) = stop_stream(client, bot_token, channel, stream_ts).await {
                        tracing::warn!("Slack relay: stopStream (rotation) failed: {}", e);
                    }
                    accumulated_stream_md.clear();
                    accumulated_raw_md.clear();
                    {
                        let mut s = slack_state.lock().await;
                        s.streaming_messages.remove(session_id);
                    }
                    *active_stream_ts = None;
                    *stream_started_at = None;
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
}
