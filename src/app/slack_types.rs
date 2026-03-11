/// A Slack message queued while waiting for a free session.
#[derive(Debug, Clone)]
pub struct PendingSlackMessage {
    pub project_idx: usize,
    pub thread_ts: String,
    pub channel: String,
    pub original_text: String,
    pub rewritten_query: Option<String>,
}
