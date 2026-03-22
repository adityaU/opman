//! Session/project switching, permission/question replies, and model selection handlers.

use leptos::prelude::*;

use crate::api::client::{api_post_void, reply_permission, reply_question, reject_question};
use crate::hooks::use_model_state::ModelRef;
use crate::hooks::use_toast::ToastExt;

use super::ChatHandlerDeps;

// ── Permission reply ───────────────────────────────────────────────

pub(super) fn build_handle_permission_reply(
    deps: ChatHandlerDeps,
) -> Callback<(String, String)> {
    Callback::new(move |(request_id, reply): (String, String)| {
        let toasts = deps.toasts;
        let set_perms = deps.sse.set_permissions;
        let rid = request_id.clone();
        leptos::task::spawn_local(async move {
            match reply_permission(&request_id, &reply).await {
                Ok(()) => {
                    set_perms.update(|v| v.retain(|p| p.id != rid));
                }
                Err(_) => toasts.add_typed("Failed to send permission reply", "error"),
            }
        });
    })
}

// ── Question reply ─────────────────────────────────────────────────

pub(super) fn build_handle_question_reply(
    deps: ChatHandlerDeps,
) -> Callback<(String, Vec<Vec<String>>)> {
    Callback::new(move |(request_id, answers): (String, Vec<Vec<String>>)| {
        let toasts = deps.toasts;
        let set_questions = deps.sse.set_questions;
        let rid = request_id.clone();
        leptos::task::spawn_local(async move {
            match reply_question(&request_id, &answers).await {
                Ok(()) => {
                    set_questions.update(|v| v.retain(|q| q.id != rid));
                }
                Err(_) => toasts.add_typed("Failed to send answer", "error"),
            }
        });
    })
}

// ── Question dismiss ───────────────────────────────────────────────

pub(super) fn build_handle_question_dismiss(deps: ChatHandlerDeps) -> Callback<String> {
    Callback::new(move |request_id: String| {
        let toasts = deps.toasts;
        let set_questions = deps.sse.set_questions;
        let rid = request_id.clone();
        leptos::task::spawn_local(async move {
            match reject_question(&request_id).await {
                Ok(()) => {
                    set_questions.update(|v| v.retain(|q| q.id != rid));
                }
                Err(_) => toasts.add_typed("Failed to dismiss question", "error"),
            }
        });
    })
}

// ── Select session ─────────────────────────────────────────────────

pub(super) fn build_handle_select_session(
    deps: ChatHandlerDeps,
) -> Callback<(String, usize)> {
    Callback::new(move |(session_id, project_idx): (String, usize)| {
        deps.set_selected_model.set(None);
        deps.set_selected_agent.set(String::new());
        // URL is the source of truth — navigate via URL
        crate::hooks::use_url_restore::navigate_to_session(
            &session_id,
            project_idx,
            &deps.panels,
        );
    })
}

// ── New session ────────────────────────────────────────────────────

pub(super) fn build_handle_new_session(deps: ChatHandlerDeps) -> Callback<()> {
    Callback::new(move |()| {
        let app_state = match deps.sse.app_state.get_untracked() {
            Some(s) => s,
            None => return,
        };
        let project_idx = app_state.active_project;
        deps.sse.expect_session_switch();
        let toasts = deps.toasts;
        let set_model = deps.set_selected_model;
        let set_agent = deps.set_selected_agent;
        let panels = deps.panels;

        leptos::task::spawn_local(async move {
            match crate::api::project::new_session(project_idx).await {
                Ok(resp) => {
                    set_model.set(None);
                    set_agent.set(String::new());
                    toasts.add_typed("New session created", "success");
                    // Navigate to the new session via URL (triggers select + state refresh).
                    crate::hooks::use_url_restore::navigate_to_session(
                        &resp.session_id,
                        project_idx,
                        &panels,
                    );
                }
                Err(_) => toasts.add_typed("Failed to create session", "error"),
            }
        });
    })
}

// ── Switch project ─────────────────────────────────────────────────

pub(super) fn build_handle_switch_project(deps: ChatHandlerDeps) -> Callback<usize> {
    let set_app_state = deps.sse.set_app_state;
    Callback::new(move |index: usize| {
        deps.sse.expect_session_switch();
        let toasts = deps.toasts;
        let set_model = deps.set_selected_model;
        let set_agent = deps.set_selected_agent;

        leptos::task::spawn_local(async move {
            match api_post_void(
                "/project/switch",
                &serde_json::json!({ "index": index }),
            )
            .await
            {
                Ok(()) => {
                    set_model.set(None);
                    set_agent.set(String::new());
                    // Fetch state as a reliable fallback.
                    if let Ok(state) = crate::api::project::fetch_app_state().await {
                        set_app_state.set(Some(state));
                    }
                }
                Err(_) => toasts.add_typed("Failed to switch project", "error"),
            }
        });
    })
}

// ── Model selected ─────────────────────────────────────────────────

pub(super) fn build_handle_model_selected(
    deps: ChatHandlerDeps,
) -> Callback<(String, String)> {
    Callback::new(move |(model_id, provider_id): (String, String)| {
        deps.set_selected_model.set(Some(ModelRef {
            provider_id,
            model_id: model_id.clone(),
        }));
        deps.toasts
            .add_typed(&format!("Model switched to {}", model_id), "success");
    })
}
