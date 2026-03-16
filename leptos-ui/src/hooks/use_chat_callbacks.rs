//! Chat callbacks hook — theme, context submit, search, workspace, autonomy, signal dismiss.
//! Matches React `useChatCallbacks.ts`.

use leptos::prelude::*;

use crate::api::client::{api_post_void, send_message};
use crate::hooks::use_notification_signals::AssistantSignal;
use crate::hooks::use_panel_state::PanelState;
use crate::hooks::use_sse_state::SseState;
use crate::hooks::use_toast::{ToastExt, ToastState};
use crate::types::api::{PersonalMemoryItem, ThemeColors, WorkspaceSnapshot};

// ── Types ──────────────────────────────────────────────────────────

/// Chat callbacks returned by `use_chat_callbacks`.
#[derive(Clone)]
pub struct ChatCallbacks {
    /// Apply a theme to CSS variables.
    pub handle_theme_applied: Callback<ThemeColors>,
    /// Send context text to the active session.
    pub handle_context_submit: Callback<String>,
    /// Update search match IDs from the search bar.
    pub handle_search_matches_changed: Callback<(Vec<String>, Option<String>)>,
    /// Show a panel error toast.
    pub handle_panel_error: Callback<String>,
    /// Restore a workspace snapshot.
    pub handle_restore_workspace: Callback<WorkspaceSnapshot>,
    /// Change the autonomy mode.
    pub on_autonomy_change: Callback<String>,
    /// Dismiss an assistant signal by ID.
    pub on_dismiss_signal: Callback<String>,
    /// Memory items filtered for the inbox.
    pub personal_memory_for_inbox: Memo<Vec<PersonalMemoryItem>>,
}

/// Dependencies for chat callbacks.
#[derive(Clone, Copy)]
pub struct ChatCallbackDeps {
    pub sse: SseState,
    pub panels: PanelState,
    pub toasts: ToastState,
    pub personal_memory: ReadSignal<Vec<PersonalMemoryItem>>,
    pub set_autonomy_mode: WriteSignal<String>,
    pub set_assistant_signals: WriteSignal<Vec<AssistantSignal>>,
    pub set_active_workspace_name: WriteSignal<Option<String>>,
    pub search_match_ids: WriteSignal<Vec<String>>,
    pub active_search_match_id: WriteSignal<Option<String>>,
    pub handle_select_session: Callback<(String, usize)>,
}

// ── Hook ───────────────────────────────────────────────────────────

/// Create chat callbacks. Call once at layout level.
pub fn use_chat_callbacks(deps: ChatCallbackDeps) -> ChatCallbacks {
    // Theme application
    let handle_theme_applied = {
        let toasts = deps.toasts;
        Callback::new(move |colors: ThemeColors| {
            apply_theme_to_css(&colors);
            toasts.add_typed("Theme applied", "success");
        })
    };

    // Context submit
    let handle_context_submit = {
        let sse = deps.sse;
        let toasts = deps.toasts;
        Callback::new(move |text: String| {
            let sid = match sse.active_session_id() {
                Some(s) => s,
                None => return,
            };
            leptos::task::spawn_local(async move {
                match send_message(&sid, &text, None).await {
                    Ok(_) => toasts.add_typed("Context sent", "success"),
                    Err(_) => toasts.add_typed("Failed to send context", "error"),
                }
            });
        })
    };

    // Search matches
    let handle_search_matches_changed = {
        let set_ids = deps.search_match_ids;
        let set_active = deps.active_search_match_id;
        Callback::new(move |(match_ids, active_id): (Vec<String>, Option<String>)| {
            set_ids.set(match_ids);
            set_active.set(active_id);
        })
    };

    // Panel error
    let handle_panel_error = {
        let toasts = deps.toasts;
        Callback::new(move |msg: String| {
            toasts.add_typed(&msg, "error");
        })
    };

    // Restore workspace
    let handle_restore_workspace = {
        let panels = deps.panels;
        let sse = deps.sse;
        let set_ws_name = deps.set_active_workspace_name;
        let handle_select = deps.handle_select_session;
        Callback::new(move |ws: WorkspaceSnapshot| {
            // Restore panel state
            panels.set_sidebar_open.set(ws.panels.sidebar);
            if ws.panels.terminal {
                panels.terminal.set_open.set(true);
            } else {
                panels.terminal.set_open.set(false);
            }
            if ws.panels.editor {
                panels.editor.set_open.set(true);
            } else {
                panels.editor.set_open.set(false);
            }
            if ws.panels.git {
                panels.git.set_open.set(true);
            } else {
                panels.git.set_open.set(false);
            }

            // Switch session if needed
            if let Some(ref sid) = ws.session_id {
                if Some(sid.clone()) != sse.active_session_id() {
                    let project_idx = sse
                        .app_state
                        .get_untracked()
                        .map(|s| s.active_project)
                        .unwrap_or(0);
                    handle_select.run((sid.clone(), project_idx));
                }
            }

            set_ws_name.set(Some(ws.name));
        })
    };

    // Autonomy change
    let on_autonomy_change = {
        let set_mode = deps.set_autonomy_mode;
        Callback::new(move |mode: String| {
            set_mode.set(mode.clone());
            leptos::task::spawn_local(async move {
                let _ = api_post_void(
                    "/assistant/autonomy",
                    &serde_json::json!({ "mode": mode }),
                )
                .await;
            });
        })
    };

    // Dismiss signal
    let on_dismiss_signal = {
        let set_signals = deps.set_assistant_signals;
        Callback::new(move |id: String| {
            set_signals.update(|v| v.retain(|s| s.id != id));
        })
    };

    // Personal memory filtered for inbox
    let personal_memory = deps.personal_memory;
    let sse = deps.sse;
    let personal_memory_for_inbox = Memo::new(move |_| {
        let memory = personal_memory.get();
        let active_project = sse.derived_active_project_idx.get();
        let active_session = sse.active_session_id();

        memory
            .into_iter()
            .filter(|item| match item.scope.as_str() {
                "global" => true,
                "project" => item.project_index == Some(active_project),
                _ => item.session_id == active_session,
            })
            .collect()
    });

    ChatCallbacks {
        handle_theme_applied,
        handle_context_submit,
        handle_search_matches_changed,
        handle_panel_error,
        handle_restore_workspace,
        on_autonomy_change,
        on_dismiss_signal,
        personal_memory_for_inbox,
    }
}

// ── Theme helpers ──────────────────────────────────────────────────

/// Apply theme colors to CSS custom properties on <html>.
fn apply_theme_to_css(colors: &ThemeColors) {
    let doc_el = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element());
    let el = match doc_el {
        Some(e) => e,
        None => return,
    };
    let style = el
        .dyn_ref::<web_sys::HtmlElement>()
        .map(|h| h.style());
    let style = match style {
        Some(s) => s,
        None => return,
    };

    let _ = style.set_property("--color-primary", &colors.primary);
    let _ = style.set_property("--color-secondary", &colors.secondary);
    let _ = style.set_property("--color-accent", &colors.accent);
    let _ = style.set_property("--color-background", &colors.background);
    let _ = style.set_property("--color-background-panel", &colors.background_panel);
    let _ = style.set_property("--color-background-element", &colors.background_element);
    let _ = style.set_property("--color-text", &colors.text);
    let _ = style.set_property("--color-text-muted", &colors.text_muted);
    let _ = style.set_property("--color-border", &colors.border);
    let _ = style.set_property("--color-border-active", &colors.border_active);
    let _ = style.set_property("--color-border-subtle", &colors.border_subtle);
    let _ = style.set_property("--color-error", &colors.error);
    let _ = style.set_property("--color-warning", &colors.warning);
    let _ = style.set_property("--color-success", &colors.success);
    let _ = style.set_property("--color-info", &colors.info);
}

use wasm_bindgen::JsCast;
