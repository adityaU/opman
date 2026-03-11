//! Session relay watcher and response batching.
//!
//! Contains the background tasks that poll OpenCode sessions for new messages
//! and relay them to Slack threads using streaming with `task_update` chunks
//! for tool progress.

mod batcher;
mod content;
mod dispatch;
mod lifecycle;
mod segments;
mod streaming;
mod watcher;

pub use batcher::spawn_response_batcher;
pub use watcher::spawn_session_relay_watcher;

use crate::blockkit::markdown_to_blocks;

use super::api::post_message_with_blocks;
use super::formatting::markdown_to_slack_mrkdwn;

/// Build the `attachments` array for Slack from table blocks.
///
/// Slack's `table` block must go in `attachments`, not top-level `blocks`.
/// Only one table per message is allowed; extras are dropped.
/// Returns `None` if there are no table blocks.
pub(crate) fn table_attachments(
    table_blocks: &[serde_json::Value],
) -> Option<Vec<serde_json::Value>> {
    if table_blocks.is_empty() {
        return None;
    }
    // Slack allows only one table per message.
    Some(vec![serde_json::json!({ "blocks": [&table_blocks[0]] })])
}

/// Post a Block Kit message to Slack, falling back to plain mrkdwn if no
/// blocks were produced.  Used by both the segment and streaming helpers.
pub(crate) async fn post_blockkit_or_plain(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    raw_md: &str,
    fallback_text: &str,
) {
    let bk = markdown_to_blocks(raw_md);
    let fallback = if fallback_text.is_empty() {
        markdown_to_slack_mrkdwn(raw_md)
    } else {
        fallback_text.to_owned()
    };
    let tbl_att = table_attachments(&bk.table_blocks);
    let att_ref = tbl_att.as_deref();
    if !bk.blocks.is_empty() || tbl_att.is_some() {
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
        let _ = super::api::post_message(client, bot_token, channel, &fallback, Some(thread_ts))
            .await;
    }
}
