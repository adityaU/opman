use crate::app::{PermissionRequest, QuestionRequest, SessionInfo, SessionMessage, TodoItem};
use crate::pty::PtyInstance;

/// Events sent from background tokio tasks back to the main event loop.
/// The event loop calls `try_recv()` each tick and dispatches to `App::handle_background_event`.
pub enum BackgroundEvent {
    /// A PTY was successfully spawned in a background (spawn_blocking) task.
    PtySpawned {
        project_idx: usize,
        session_id: String,
        pty: PtyInstance,
    },
    /// Sessions were fetched for a project.
    SessionsFetched {
        project_idx: usize,
        sessions: Vec<SessionInfo>,
    },
    /// Session fetch failed (non-fatal, just skip).
    SessionFetchFailed { project_idx: usize },
    /// A session was selected via the API (pending_session_select completed).
    SessionSelected {
        project_idx: usize,
        session_id: String,
    },
    /// A project was fully activated (server healthy + PTY spawned).
    ProjectActivated { project_idx: usize },
    /// SSE: a new session was created on the server.
    SseSessionCreated {
        project_idx: usize,
        session: SessionInfo,
    },
    /// SSE: a session was updated (title changed, etc.).
    SseSessionUpdated {
        project_idx: usize,
        session: SessionInfo,
    },
    /// SSE: a session was deleted on the server.
    SseSessionDeleted {
        project_idx: usize,
        session_id: String,
    },
    /// SSE: a session became idle.
    SseSessionIdle {
        #[allow(dead_code)]
        project_idx: usize,
        session_id: String,
    },
    /// SSE: a session became busy (actively processing).
    SseSessionBusy { session_id: String },
    /// SSE: a file was edited by the AI agent.
    SseFileEdited {
        project_idx: usize,
        file_path: String,
    },
    /// Todos fetched via REST API.
    TodosFetched {
        session_id: String,
        todos: Vec<TodoItem>,
    },
    /// SSE: todo list updated for a session.
    SseTodoUpdated {
        session_id: String,
        todos: Vec<TodoItem>,
    },
    /// SSE: message.updated with cost/token data for a session.
    SseMessageUpdated {
        session_id: String,
        cost: f64,
        input_tokens: u64,
        output_tokens: u64,
        reasoning_tokens: u64,
        cache_read: u64,
        cache_write: u64,
    },
    /// SSE: a permission was requested by the AI agent.
    SsePermissionAsked {
        project_idx: usize,
        request: PermissionRequest,
    },
    /// SSE: a question was asked by the AI agent.
    SseQuestionAsked {
        project_idx: usize,
        request: QuestionRequest,
    },
    /// Provider model limits fetched from REST API.
    ModelLimitsFetched {
        project_idx: usize,
        context_window: u64,
    },
    /// MCP socket request from a bridge process (terminal tool invocation).
    McpSocketRequest {
        project_idx: usize,
        session_id: String,
        pending: crate::mcp::PendingSocketRequest,
    },
    /// User messages fetched for the watcher modal "re-inject original" picker.
    WatcherSessionMessages {
        session_id: String,
        messages: Vec<SessionMessage>,
    },
    /// Session status fetched from REST API (busy/retry sessions).
    SessionStatusFetched {
        /// Map of session_id -> status_type ("busy", "retry").
        /// Sessions absent from the map are idle.
        busy_sessions: Vec<String>,
    },
    /// Event from the Slack integration subsystem.
    SlackEvent(crate::slack::SlackBackgroundEvent),
}
