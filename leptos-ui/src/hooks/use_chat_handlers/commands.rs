//! Send, abort, agent-change, and slash-command handlers.

use leptos::prelude::*;

use crate::api::client::{self, api_post_void, abort_session};
use crate::hooks::use_toast::ToastExt;

use super::{inject_memory_guidance, modal_for_command, ChatHandlerDeps};

// ── Send ───────────────────────────────────────────────────────────

pub(super) fn build_handle_send(deps: ChatHandlerDeps) -> Callback<(String, Option<Vec<String>>)> {
    Callback::new(move |(text, images): (String, Option<Vec<String>>)| {
        let sid = match deps.sse.active_session_id() {
            Some(s) => s,
            None => return,
        };
        if deps.sending.get_untracked() {
            return;
        }
        deps.set_sending.set(true);
        deps.sse.add_optimistic_message(&text);

        let memory = deps.active_memory_items.get_untracked();
        let enriched = inject_memory_guidance(&text, &memory);
        let toasts = deps.toasts;
        let set_sending = deps.set_sending;

        leptos::task::spawn_local(async move {
            let result = client::send_message(&sid, &enriched, images).await;
            if result.is_err() {
                toasts.add_typed("Failed to send message", "error");
            }
            set_sending.set(false);
        });
    })
}

// ── Abort ──────────────────────────────────────────────────────────

pub(super) fn build_handle_abort(deps: ChatHandlerDeps) -> Callback<()> {
    Callback::new(move |()| {
        let sid = match deps.sse.active_session_id() {
            Some(s) => s,
            None => return,
        };
        let toasts = deps.toasts;
        leptos::task::spawn_local(async move {
            match abort_session(&sid).await {
                Ok(()) => toasts.add_typed("Session aborted", "info"),
                Err(_) => toasts.add_typed("Failed to abort session", "error"),
            }
        });
    })
}

// ── Agent change ───────────────────────────────────────────────────

pub(super) fn build_handle_agent_change(deps: ChatHandlerDeps) -> Callback<String> {
    Callback::new(move |agent_id: String| {
        deps.set_selected_agent.set(agent_id.clone());
        if let Some(sid) = deps.sse.active_session_id() {
            let agent = agent_id.clone();
            leptos::task::spawn_local(async move {
                let _ = api_post_void(
                    &format!(
                        "/session/{}/command",
                        js_sys::encode_uri_component(&sid)
                    ),
                    &serde_json::json!({ "command": "agent", "args": agent }),
                )
                .await;
            });
        }
        deps.toasts
            .add_typed(&format!("Agent switched to {}", agent_id), "success");
    })
}

// ── Slash command dispatch ─────────────────────────────────────────

pub(super) fn build_handle_command(
    deps: ChatHandlerDeps,
) -> Callback<(String, Option<String>)> {
    Callback::new(move |(command, args): (String, Option<String>)| {
        let cmd = command.as_str();

        // /cancel
        if cmd == "cancel" {
            let sid = match deps.sse.active_session_id() {
                Some(s) => s,
                None => return,
            };
            let toasts = deps.toasts;
            leptos::task::spawn_local(async move {
                match abort_session(&sid).await {
                    Ok(()) => toasts.add_typed("Session cancelled", "info"),
                    Err(_) => toasts.add_typed("Failed to cancel session", "error"),
                }
            });
            return;
        }

        // /new
        if cmd == "new" {
            let app_state = match deps.sse.app_state.get_untracked() {
                Some(s) => s,
                None => return,
            };
            let project_idx = app_state.active_project;
            deps.sse.expect_session_switch();
            let toasts = deps.toasts;
            let set_model = deps.set_selected_model;
            let set_agent = deps.set_selected_agent;
            let set_app_state = deps.sse.set_app_state;
            leptos::task::spawn_local(async move {
                match api_post_void(
                    "/session/new",
                    &serde_json::json!({ "project_idx": project_idx }),
                )
                .await
                {
                    Ok(()) => {
                        set_model.set(None);
                        set_agent.set(String::new());
                        toasts.add_typed("New session created", "success");
                        if let Ok(state) = crate::api::project::fetch_app_state().await {
                            set_app_state.set(Some(state));
                        }
                    }
                    Err(_) => toasts.add_typed("Failed to create session", "error"),
                }
            });
            return;
        }

        // Toggle commands
        if cmd == "terminal" {
            deps.panels.terminal.toggle();
            return;
        }
        if cmd == "neovim" || cmd == "nvim" {
            deps.panels.editor.toggle();
            return;
        }
        if cmd == "git" {
            deps.panels.git.toggle();
            return;
        }

        // Modal commands
        if let Some(modal_name) = modal_for_command(cmd) {
            deps.modals.open_str(modal_name);
            return;
        }

        // Fallback: server-side command
        let sid = match deps.sse.active_session_id() {
            Some(s) => s,
            None => return,
        };
        let command_owned = command.clone();
        let args_owned = args;
        let toasts = deps.toasts;
        leptos::task::spawn_local(async move {
            let payload = match args_owned {
                Some(a) => serde_json::json!({ "command": command_owned, "args": a }),
                None => serde_json::json!({ "command": command_owned }),
            };
            match api_post_void(
                &format!(
                    "/session/{}/command",
                    js_sys::encode_uri_component(&sid)
                ),
                &payload,
            )
            .await
            {
                Ok(()) => {}
                Err(_) => {
                    toasts.add_typed(&format!("Command /{} failed", command_owned), "error")
                }
            }
        });
    })
}
