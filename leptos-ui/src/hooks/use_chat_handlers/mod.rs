//! Chat handler hook — send, abort, command dispatch, permission/question replies, session management.
//! Matches React `useChatHandlers.ts` + `chatLayoutHandlers.ts`.

mod commands;
mod session_actions;
pub(crate) mod transcript;

use leptos::prelude::*;

use crate::hooks::use_model_state::ModelRef;
use crate::hooks::use_modal_state::ModalState;
use crate::hooks::use_panel_state::PanelState;
use crate::hooks::use_sse_state::SseState;
use crate::hooks::use_toast::ToastState;
use crate::types::api::PersonalMemoryItem;

// ── Command → modal mapping ────────────────────────────────────────

fn modal_for_command(cmd: &str) -> Option<&'static str> {
    match cmd {
        "models" | "model" => Some("modelPicker"),
        "agent" => Some("agentPicker"),
        "theme" => Some("themeSelector"),
        "keys" | "keybindings" => Some("cheatsheet"),
        "todos" => Some("todoPanel"),
        "sessions" => Some("sessionSelector"),
        "context" => Some("contextInput"),
        "settings" => Some("settings"),
        "watcher" => Some("watcher"),
        "context-window" => Some("contextWindow"),
        "diff-review" => Some("diffReview"),
        "search" => Some("searchBar"),
        "cross-search" => Some("crossSearch"),
        "session-graph" => Some("sessionGraph"),
        "session-dashboard" => Some("sessionDashboard"),
        "activity-feed" => Some("activityFeed"),
        "notification-prefs" => Some("notificationPrefs"),
        "assistant-center" => Some("assistantCenter"),
        "inbox" => Some("inbox"),
        "memory" => Some("memory"),
        "autonomy" => Some("autonomy"),
        "routines" => Some("routines"),
        "delegation" => Some("delegation"),
        "missions" => Some("missions"),
        "workspaces" | "workspace" => Some("workspaceManager"),
        "system" | "htop" | "monitor" => Some("systemMonitor"),
        _ => None,
    }
}

fn is_toggle_command(cmd: &str) -> bool {
    matches!(cmd, "terminal" | "neovim" | "nvim" | "git" | "split-view")
}

/// Check if a command is handled locally (no server round-trip).
pub fn is_local_command(cmd: &str) -> bool {
    cmd == "new" || cmd == "cancel" || modal_for_command(cmd).is_some() || is_toggle_command(cmd)
}

// ── Memory injection helper ────────────────────────────────────────

fn inject_memory_guidance(text: &str, memory_items: &[PersonalMemoryItem]) -> String {
    if memory_items.is_empty() {
        return text.to_string();
    }
    let guidance: Vec<String> = memory_items
        .iter()
        .take(5)
        .map(|item| format!("- {}: {}", item.label, item.content))
        .collect();
    format!(
        "[Assistant memory in effect]\n{}\n\n[User request]\n{}",
        guidance.join("\n"),
        text
    )
}

// ── Types ──────────────────────────────────────────────────────────

/// Chat handlers returned by `use_chat_handlers`.
#[derive(Clone)]
pub struct ChatHandlers {
    pub handle_send: Callback<(String, Option<Vec<String>>)>,
    pub handle_abort: Callback<()>,
    pub handle_agent_change: Callback<String>,
    pub handle_command: Callback<(String, Option<String>)>,
    pub handle_permission_reply: Callback<(String, String)>,
    pub handle_question_reply: Callback<(String, Vec<Vec<String>>)>,
    pub handle_question_dismiss: Callback<String>,
    pub handle_select_session: Callback<(String, usize)>,
    pub handle_new_session: Callback<()>,
    pub handle_switch_project: Callback<usize>,
    pub handle_model_selected: Callback<(String, String)>,
}

/// Dependencies needed by the chat handlers.
#[derive(Clone, Copy)]
pub struct ChatHandlerDeps {
    pub sse: SseState,
    pub panels: PanelState,
    pub modals: ModalState,
    pub toasts: ToastState,
    pub selected_model: ReadSignal<Option<ModelRef>>,
    pub set_selected_model: WriteSignal<Option<ModelRef>>,
    pub selected_agent: ReadSignal<String>,
    pub set_selected_agent: WriteSignal<String>,
    /// Derived current agent (selection > last message > default).
    pub current_agent: Memo<String>,
    pub sending: ReadSignal<bool>,
    pub set_sending: WriteSignal<bool>,
    pub active_memory_items: ReadSignal<Vec<PersonalMemoryItem>>,
}

// ── Hook ───────────────────────────────────────────────────────────

/// Create chat handler callbacks. Call once at layout level.
pub fn use_chat_handlers(deps: ChatHandlerDeps) -> ChatHandlers {
    let handle_send = commands::build_handle_send(deps);
    let handle_abort = commands::build_handle_abort(deps);
    let handle_agent_change = commands::build_handle_agent_change(deps);
    let handle_command = commands::build_handle_command(deps);

    let handle_permission_reply = session_actions::build_handle_permission_reply(deps);
    let handle_question_reply = session_actions::build_handle_question_reply(deps);
    let handle_question_dismiss = session_actions::build_handle_question_dismiss(deps);
    let handle_select_session = session_actions::build_handle_select_session(deps);
    let handle_new_session = session_actions::build_handle_new_session(deps);
    let handle_switch_project = session_actions::build_handle_switch_project(deps);
    let handle_model_selected = session_actions::build_handle_model_selected(deps);

    ChatHandlers {
        handle_send,
        handle_abort,
        handle_agent_change,
        handle_command,
        handle_permission_reply,
        handle_question_reply,
        handle_question_dismiss,
        handle_select_session,
        handle_new_session,
        handle_switch_project,
        handle_model_selected,
    }
}
