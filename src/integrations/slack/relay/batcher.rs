//! Response batching – periodically flushes pending response buffers to Slack.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::{debug, error};

use crate::blockkit::markdown_to_blocks;

use super::super::api::{chunk_for_slack, post_message, post_message_with_blocks};
use super::super::formatting::markdown_to_slack_mrkdwn;
use super::super::state::SlackState;
use super::super::types::SlackLogLevel;
use super::super::auth::SlackAuth;
use super::table_attachments;

/// Spawn a background task that periodically checks for pending response batches
/// and sends them to Slack.
pub async fn spawn_response_batcher(
    auth: SlackAuth,
    state: Arc<Mutex<SlackState>>,
    batch_interval_secs: u64,
    _event_tx: tokio::sync::mpsc::UnboundedSender<super::super::types::SlackBackgroundEvent>,
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
