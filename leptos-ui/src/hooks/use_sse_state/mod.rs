//! SSE state management — reactive store bridging SSE events to Leptos signals.
//! Matches the React `useSSE` hook family.
//!
//! Split into sub-modules:
//! - `types`: ConnectionStatus, SessionStatus enums
//! - `messages`: message map mutations, flush, fetch, pagination
//! - `tracking`: active session tracking + session-switch guard

mod types;
mod messages;
mod subagent_messages;
mod tracking;

pub use types::{ConnectionStatus, SessionStatus};

use crate::types::api::{AppState, SessionStats};
use crate::types::core::{Message, PermissionRequest, QuestionRequest};
use crate::types::events::WatcherStatus;
use crate::sse::message_map::MessageMap;
use leptos::prelude::*;
use std::collections::{HashMap, HashSet};

/// Full SSE-driven application state store.
#[derive(Clone, Copy)]
pub struct SseState {
    // Core state
    pub app_state: ReadSignal<Option<AppState>>,
    pub set_app_state: WriteSignal<Option<AppState>>,

    // ── Derived signals (narrow subscriptions — avoid reading raw app_state) ──
    /// Active project index — only changes when the project index changes.
    pub derived_active_project_idx: Memo<usize>,
    /// Active project info — only changes when project fields change (name/path/sessions/etc.).
    pub derived_active_project: Memo<Option<crate::types::api::ProjectInfo>>,
    /// Project list — only changes when the projects Vec changes.
    pub derived_projects: Memo<Vec<crate::types::api::ProjectInfo>>,

    pub messages: ReadSignal<Vec<Message>>,
    pub set_messages: WriteSignal<Vec<Message>>,

    pub stats: ReadSignal<Option<SessionStats>>,
    pub set_stats: WriteSignal<Option<SessionStats>>,

    pub busy_sessions: ReadSignal<HashSet<String>>,
    pub set_busy_sessions: WriteSignal<HashSet<String>>,

    pub error_sessions: ReadSignal<HashSet<String>>,
    pub set_error_sessions: WriteSignal<HashSet<String>>,

    pub input_sessions: ReadSignal<HashSet<String>>,
    pub set_input_sessions: WriteSignal<HashSet<String>>,

    pub unseen_sessions: ReadSignal<HashSet<String>>,
    pub set_unseen_sessions: WriteSignal<HashSet<String>>,

    pub permissions: ReadSignal<Vec<PermissionRequest>>,
    pub set_permissions: WriteSignal<Vec<PermissionRequest>>,

    pub questions: ReadSignal<Vec<QuestionRequest>>,
    pub set_questions: WriteSignal<Vec<QuestionRequest>>,

    // Session
    pub session_status: ReadSignal<SessionStatus>,
    pub set_session_status: WriteSignal<SessionStatus>,

    pub connection_status: ReadSignal<ConnectionStatus>,
    pub set_connection_status: WriteSignal<ConnectionStatus>,

    // Loading
    pub is_loading_messages: ReadSignal<bool>,
    pub set_is_loading_messages: WriteSignal<bool>,

    pub is_loading_older: ReadSignal<bool>,
    pub set_is_loading_older: WriteSignal<bool>,

    pub has_older_messages: ReadSignal<bool>,
    pub set_has_older_messages: WriteSignal<bool>,

    pub total_message_count: ReadSignal<usize>,
    pub set_total_message_count: WriteSignal<usize>,

    // Watcher
    pub watcher_status: ReadSignal<Option<WatcherStatus>>,
    pub set_watcher_status: WriteSignal<Option<WatcherStatus>>,

    // File edits
    pub file_edit_count: ReadSignal<usize>,
    pub set_file_edit_count: WriteSignal<usize>,

    // Presence
    pub presence_clients: ReadSignal<Vec<crate::types::api::ClientPresence>>,
    pub set_presence_clients: WriteSignal<Vec<crate::types::api::ClientPresence>>,

    // Internal: active session tracking (using RwSignals for Send+Sync)
    /// The internal message map — for SSE event handlers to mutate.
    pub(crate) message_map: RwSignal<MessageMap>,
    /// The last known active session ID, for session-switch guard logic.
    pub(crate) active_session_id_stored: RwSignal<Option<String>>,
    /// Session generation counter — incremented on each session switch.
    pub(crate) session_gen: RwSignal<u32>,
    /// When true, the next appState update is allowed to change the active session.
    pub(crate) expect_switch: RwSignal<bool>,
    /// Whether a flush is already scheduled for the next animation frame.
    pub(crate) flush_pending: RwSignal<bool>,
    /// Handle to the pending requestAnimationFrame, for cancellation.
    pub(crate) raf_handle: RwSignal<i32>,
    /// Guard to prevent concurrent `refresh_messages()` calls.
    pub(crate) is_refreshing: RwSignal<bool>,

    // ── Subagent message state ──
    /// Per-session message maps for non-active (subagent) sessions.
    pub(crate) subagent_maps: RwSignal<HashMap<String, MessageMap>>,
    /// Rendered subagent messages: session_id → sorted Vec<Message>.
    pub subagent_messages: ReadSignal<HashMap<String, Vec<Message>>>,
    pub set_subagent_messages: WriteSignal<HashMap<String, Vec<Message>>>,
    /// Whether a subagent flush is already scheduled.
    pub(crate) subagent_flush_pending: RwSignal<bool>,
    /// Handle to the pending subagent rAF.
    pub(crate) subagent_raf_handle: RwSignal<i32>,
}

impl SseState {
    /// Get the active project info.
    /// Subscribes to the derived memo (narrower than raw `app_state`).
    pub fn active_project(&self) -> Option<crate::types::api::ProjectInfo> {
        self.derived_active_project.get()
    }

    /// Get the active session ID from app state.
    /// Subscribes to the derived active-project memo.
    pub fn active_session_id(&self) -> Option<String> {
        self.derived_active_project
            .get()
            .and_then(|p| p.active_session.clone())
    }

    /// Get the tracked active session ID (may differ from app_state during guard).
    /// This is **untracked** — use `tracked_session_id_reactive()` to subscribe.
    pub fn tracked_session_id(&self) -> Option<String> {
        self.active_session_id_stored.get_untracked()
    }

    /// Get the tracked active session ID **reactively**.
    /// Subscribe to this for URL sync / UI that should update only on session switches.
    pub fn tracked_session_id_reactive(&self) -> Option<String> {
        self.active_session_id_stored.get()
    }

    /// Get the current session generation counter.
    pub fn current_gen(&self) -> u32 {
        self.session_gen.get_untracked()
    }

    /// Get a clone of the message map for reading.
    pub fn message_map_snapshot(&self) -> MessageMap {
        self.message_map.get_untracked()
    }

    /// Signal that a user-initiated session switch is expected.
    /// Call before selectSession/newSession/switchProject so the next
    /// appState update is allowed to change the active session.
    pub fn expect_session_switch(&self) {
        self.expect_switch.set(true);
    }
}

/// Create the SSE state store. Call once at the layout level.
pub fn use_sse_state() -> SseState {
    let (app_state, set_app_state) = signal::<Option<AppState>>(None);
    let (messages, set_messages) = signal::<Vec<Message>>(Vec::new());
    let (stats, set_stats) = signal::<Option<SessionStats>>(None);
    let (busy_sessions, set_busy_sessions) = signal::<HashSet<String>>(HashSet::new());
    let (error_sessions, set_error_sessions) = signal::<HashSet<String>>(HashSet::new());
    let (input_sessions, set_input_sessions) = signal::<HashSet<String>>(HashSet::new());
    let (unseen_sessions, set_unseen_sessions) = signal::<HashSet<String>>(HashSet::new());
    let (permissions, set_permissions) = signal::<Vec<PermissionRequest>>(Vec::new());
    let (questions, set_questions) = signal::<Vec<QuestionRequest>>(Vec::new());
    let (session_status, set_session_status) = signal(SessionStatus::Idle);
    let (connection_status, set_connection_status) = signal(ConnectionStatus::Disconnected);
    let (is_loading_messages, set_is_loading_messages) = signal(false);
    let (is_loading_older, set_is_loading_older) = signal(false);
    let (has_older_messages, set_has_older_messages) = signal(false);
    let (total_message_count, set_total_message_count) = signal(0usize);
    let (watcher_status, set_watcher_status) = signal::<Option<WatcherStatus>>(None);
    let (file_edit_count, set_file_edit_count) = signal(0usize);
    let (presence_clients, set_presence_clients) =
        signal(Vec::<crate::types::api::ClientPresence>::new());

    let message_map = RwSignal::new(MessageMap::new());
    let active_session_id_stored = RwSignal::new(None::<String>);
    let session_gen = RwSignal::new(0u32);
    let expect_switch = RwSignal::new(false);
    let flush_pending = RwSignal::new(false);
    let raf_handle = RwSignal::new(0i32);
    let is_refreshing = RwSignal::new(false);

    let subagent_maps = RwSignal::new(HashMap::<String, MessageMap>::new());
    let (subagent_messages, set_subagent_messages) =
        signal(HashMap::<String, Vec<Message>>::new());
    let subagent_flush_pending = RwSignal::new(false);
    let subagent_raf_handle = RwSignal::new(0i32);

    // ── Derived memos — narrow subscriptions to avoid full app_state fan-out ──
    let derived_active_project_idx = Memo::new(move |_| {
        app_state.get().map(|s| s.active_project).unwrap_or(0)
    });
    let derived_active_project = Memo::new(move |_| {
        app_state
            .get()
            .as_ref()
            .and_then(|s| s.projects.get(s.active_project).cloned())
    });
    let derived_projects = Memo::new(move |_| {
        app_state
            .get()
            .map(|s| s.projects.clone())
            .unwrap_or_default()
    });

    SseState {
        app_state,
        set_app_state,
        derived_active_project_idx,
        derived_active_project,
        derived_projects,
        messages,
        set_messages,
        stats,
        set_stats,
        busy_sessions,
        set_busy_sessions,
        error_sessions,
        set_error_sessions,
        input_sessions,
        set_input_sessions,
        unseen_sessions,
        set_unseen_sessions,
        permissions,
        set_permissions,
        questions,
        set_questions,
        session_status,
        set_session_status,
        connection_status,
        set_connection_status,
        is_loading_messages,
        set_is_loading_messages,
        is_loading_older,
        set_is_loading_older,
        has_older_messages,
        set_has_older_messages,
        total_message_count,
        set_total_message_count,
        watcher_status,
        set_watcher_status,
        file_edit_count,
        set_file_edit_count,
        presence_clients,
        set_presence_clients,
        message_map,
        active_session_id_stored,
        session_gen,
        expect_switch,
        flush_pending,
        raf_handle,
        is_refreshing,
        subagent_maps,
        subagent_messages,
        set_subagent_messages,
        subagent_flush_pending,
        subagent_raf_handle,
    }
}
