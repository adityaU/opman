//! Slack runtime state management.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Notify;

use super::types::{SlackConnectionStatus, SlackLogEntry, SlackLogLevel, SlackMetrics};

/// State maintained by the Slack subsystem.
#[allow(dead_code)]
pub struct SlackState {
    /// Current connection status.
    pub status: SlackConnectionStatus,
    /// Mapping from Slack thread_ts → (project_idx, session_id).
    /// Tracks which opman session each Slack thread is routed to.
    pub thread_sessions: HashMap<String, (usize, String)>,
    /// Pending response batches: session_id → accumulated text.
    pub response_buffers: HashMap<String, String>,
    /// Mapping from session_id → (channel, thread_ts) for response relay.
    pub session_threads: HashMap<String, (String, String)>,
    /// Sessions for which we have already relayed the assistant response.
    /// Prevents duplicate posts when SseSessionIdle fires multiple times.
    pub relayed_sessions: HashSet<String>,
    /// Number of messages in a session at the time Slack routed a message to it.
    /// Used to only relay messages that came after the routing point.
    pub session_msg_offset: HashMap<String, usize>,
    /// Abort handles for live relay watchers (session_id → AbortHandle).
    /// Used to cancel watchers when sessions are removed or app shuts down.
    pub relay_abort_handles: HashMap<String, tokio::task::AbortHandle>,
    /// Active streaming message timestamps: session_id → stream message ts.
    /// Used by the relay watcher to append content to an active stream.
    pub streaming_messages: HashMap<String, String>,
    /// Subagent session → (channel, thread_ts, parent_session_id) for the
    /// subagent's dedicated Slack thread.  When a subagent session is created,
    /// we post a new top-level message and route the subagent's relay output to
    /// that thread.  The parent relay watcher uses this to replace "task" tool
    /// output with a link to the child thread.
    pub subagent_threads: HashMap<String, (String, String, String)>,
    /// Notify handles for relay watchers: session_id → Arc<Notify>.
    /// SSE message events call `.notify_one()` to wake the relay watcher
    /// immediately instead of waiting for the next poll interval.
    pub relay_notifiers: HashMap<String, Arc<Notify>>,
    /// Message timestamp of the "living" todo checklist message posted in
    /// the Slack thread for each session.  Used to `chat.update` the same
    /// message each time the todo list changes.
    pub todo_message_ts: HashMap<String, String>,
    /// Pending permission requests: thread_ts → (request_id, session_id, project_idx, message_ts, PermissionRequest).
    /// When a thread reply or button click arrives, we check this map to see if the reply is
    /// a permission response ("once", "always", "reject").
    pub pending_permissions:
        HashMap<String, (String, String, usize, String, crate::app::PermissionRequest)>,
    /// Pending question requests: thread_ts → (request_id, session_id, project_idx, message_ts, QuestionRequest).
    /// When a thread reply or button click arrives, we check this map to see if the reply is
    /// a question answer (option numbers or custom text).
    pub pending_questions:
        HashMap<String, (String, String, usize, String, crate::app::QuestionRequest)>,
    /// Slack event log for debugging.
    pub event_log: Vec<SlackLogEntry>,
    /// Metrics counters.
    pub metrics: SlackMetrics,
    /// Sessions for which the SseSessionIdle handler has requested the relay
    /// watcher to stop the active stream.  The relay watcher owns the stream
    /// lifecycle; external code should set this flag and notify the watcher
    /// instead of calling stop_stream directly.
    pub stream_stop_requested: HashSet<String>,
    /// Active relay: maps thread_ts → session_id.  At most one relay can be
    /// attached per Slack thread.  When a new relay is connected, the previous
    /// one is detached (its watcher is aborted and state cleaned up).
    pub active_relay: HashMap<String, String>,
}

impl SlackState {
    pub fn new() -> Self {
        Self {
            status: SlackConnectionStatus::Disconnected,
            thread_sessions: HashMap::new(),
            response_buffers: HashMap::new(),
            session_threads: HashMap::new(),
            relayed_sessions: HashSet::new(),
            session_msg_offset: HashMap::new(),
            relay_abort_handles: HashMap::new(),
            streaming_messages: HashMap::new(),
            subagent_threads: HashMap::new(),
            relay_notifiers: HashMap::new(),

            todo_message_ts: HashMap::new(),
            pending_permissions: HashMap::new(),
            pending_questions: HashMap::new(),
            event_log: Vec::new(),
            metrics: SlackMetrics::default(),
            stream_stop_requested: HashSet::new(),
            active_relay: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn log(&mut self, level: SlackLogLevel, message: String) {
        self.event_log.push(SlackLogEntry {
            timestamp: Instant::now(),
            level,
            message,
        });
        // Keep only the last 200 entries.
        if self.event_log.len() > 200 {
            self.event_log.drain(0..self.event_log.len() - 200);
        }
    }

    /// Wake up the relay watcher for a session so it polls immediately.
    pub fn notify_relay(&self, session_id: &str) {
        if let Some(n) = self.relay_notifiers.get(session_id) {
            n.notify_one();
        }
    }

    /// Signal the relay watcher for `session_id` to stop its active stream.
    /// This avoids a race condition where SseSessionIdle calls stop_stream
    /// concurrently with the relay watcher, which can cause duplicate Slack
    /// message bubbles.
    pub fn request_stream_stop(&mut self, session_id: &str) {
        self.stream_stop_requested.insert(session_id.to_string());
        self.notify_relay(session_id);
    }

    /// Detach the current relay from a Slack thread.  Aborts the old relay
    /// watcher, removes state mappings, and returns the old session_id (if
    /// any).  Call this *before* attaching a new relay to the same thread.
    pub fn detach_relay(&mut self, thread_ts: &str) -> Option<String> {
        let old_session_id = self.active_relay.remove(thread_ts)?;

        // Abort the old watcher.
        if let Some(handle) = self.relay_abort_handles.remove(&old_session_id) {
            handle.abort();
            tracing::info!(
                "Slack: detached relay for session {} from thread {}",
                &old_session_id[..8.min(old_session_id.len())],
                thread_ts
            );
        }

        // Clean up ancillary state for the old session.
        self.relay_notifiers.remove(&old_session_id);
        self.streaming_messages.remove(&old_session_id);
        self.session_msg_offset.remove(&old_session_id);
        self.stream_stop_requested.remove(&old_session_id);
        self.relayed_sessions.remove(&old_session_id);

        // Remove the reverse mappings only if they still point at this thread.
        if let Some((_, ts)) = self.session_threads.get(&old_session_id) {
            if ts == thread_ts {
                self.session_threads.remove(&old_session_id);
            }
        }
        if let Some((_, sid)) = self.thread_sessions.get(thread_ts) {
            if *sid == old_session_id {
                self.thread_sessions.remove(thread_ts);
            }
        }

        Some(old_session_id)
    }

    /// Detach any existing relay for a given **session**, regardless of which
    /// thread it's attached to.  This handles the case where a new thread
    /// routes to the same session — `detach_relay(thread_ts)` wouldn't find
    /// the old relay because it's keyed on the *old* thread_ts.
    ///
    /// Returns `Some((old_thread_ts, session_id))` if a relay was detached.
    pub fn detach_relay_by_session(&mut self, session_id: &str) -> Option<(String, String)> {
        // Find which thread (if any) currently has an active relay for this session.
        let old_thread_ts = self
            .active_relay
            .iter()
            .find(|(_, sid)| sid.as_str() == session_id)
            .map(|(ts, _)| ts.clone())?;

        // Delegate to the existing detach_relay which handles all cleanup.
        let old_sid = self.detach_relay(&old_thread_ts)?;
        Some((old_thread_ts, old_sid))
    }
}

/// Construct a Slack thread permalink from channel and thread_ts.
///
/// Format: `https://slack.com/archives/{channel}/p{ts_without_dot}`
pub fn slack_thread_link(channel: &str, thread_ts: &str) -> String {
    let ts_nodot = thread_ts.replace('.', "");
    format!("https://slack.com/archives/{}/p{}", channel, ts_nodot)
}
