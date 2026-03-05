//! Slack event types, enums, log entries, and metrics.

use std::time::Instant;

use super::auth::SlackAuth;

// ── Socket Mode Events ─────────────────────────────────────────────────

/// Events sent from the Slack subsystem to the main app event loop.
#[derive(Debug)]
#[allow(dead_code)] // Variants and fields used at runtime via background event channel.
pub enum SlackBackgroundEvent {
    /// A new top-level message arrived; needs AI triage for project detection.
    IncomingMessage {
        text: String,
        channel: String,
        ts: String,
        user: String,
    },
    /// A thread reply arrived; route to existing session.
    IncomingThreadReply {
        text: String,
        channel: String,
        ts: String,
        thread_ts: String,
        user: String,
    },
    /// AI triage completed: identified target project and optional model.
    TriageResult {
        thread_ts: String,
        channel: String,
        original_text: String,
        /// The query rewritten by triage AI for the target project session.
        rewritten_query: Option<String>,
        project_path: Option<String>,
        model: Option<String>,
        /// A direct answer from the triage AI for informational queries
        /// (e.g., listing sessions, project status). When present, the answer
        /// is posted to Slack without routing to any project session.
        direct_answer: Option<String>,
        /// When true, the user explicitly requested creating a new session.
        /// The system will spawn a new session and queue the message for it.
        create_session: bool,
        /// When true, the user only wants to connect/attach to a session
        /// without sending any message. The thread will be linked to the
        /// session's relay but no user message is forwarded.
        connect_only: bool,
        error: Option<String>,
    },
    /// Response batch ready to send to Slack.
    ResponseBatch {
        channel: String,
        thread_ts: String,
        text: String,
    },
    /// OAuth flow completed.
    OAuthComplete(anyhow::Result<SlackAuth>),
    /// Socket Mode connection status changed.
    ConnectionStatus(SlackConnectionStatus),
    /// A Slack Block Kit interactive button was clicked.
    BlockAction {
        /// The `action_id` from the button (e.g. "perm_once:<req_id>").
        action_id: String,
        /// Channel where the interaction occurred.
        channel: String,
        /// Message ts that contained the button (for updating the message).
        message_ts: String,
        /// Thread ts (if the message was inside a thread).
        thread_ts: Option<String>,
        /// The user who clicked.
        user: String,
    },
    /// A Slack slash command was invoked (e.g. `/opman-projects`).
    SlashCommand {
        /// The command name (e.g. "/opman-projects").
        command: String,
        /// The text after the command (e.g. "myproject" for `/opman-sessions myproject`).
        text: String,
        /// Channel where the command was invoked.
        channel: String,
        /// The user who invoked the command.
        user: String,
        /// Response URL for deferred responses (valid for 30 minutes).
        response_url: String,
        /// Trigger ID for opening modals (valid for 3 seconds).
        trigger_id: String,
    },
    /// A Slack modal (view) was submitted by the user.
    ViewSubmission {
        /// The `callback_id` set when the modal was opened (identifies which modal).
        callback_id: String,
        /// The user who submitted the modal.
        user: String,
        /// Extracted values from `view.state.values`.
        /// Structure: `{ block_id: { action_id: { type, value/selected_option, ... } } }`
        values: serde_json::Value,
        /// Private metadata string passed through from modal creation.
        private_metadata: String,
        /// Trigger ID from the submission (valid ~3 s, can open/update modals).
        trigger_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlackConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
    AuthError(String),
}

// ── Log & Metrics ───────────────────────────────────────────────────────

/// A single log entry in the Slack event log.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SlackLogEntry {
    pub timestamp: Instant,
    pub level: SlackLogLevel,
    pub message: String,
}

/// Severity level for Slack log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SlackLogLevel {
    Info,
    Warn,
    Error,
}

/// Metrics tracked by the Slack subsystem.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SlackMetrics {
    /// Number of messages routed to sessions.
    pub messages_routed: u64,
    /// Number of messages where triage failed.
    pub triage_failures: u64,
    /// Number of thread replies injected.
    pub thread_replies: u64,
    /// Number of response batches sent to Slack.
    pub batches_sent: u64,
    /// Number of reconnections.
    pub reconnections: u64,
    /// Timestamp of last successful message route.
    pub last_routed_at: Option<Instant>,
}

// ── Session Metadata ────────────────────────────────────────────────────

/// Metadata about a session, passed to @ command handlers.
#[derive(Clone, Debug)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub parent_id: String,
    pub updated: u64,
    pub project_idx: usize,
    pub project_name: String,
    pub project_dir: String,
}
