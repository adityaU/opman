//! Sidebar callback builders — session/project switching, delete, rename, pin.

use std::collections::HashSet;

use leptos::prelude::*;

use crate::hooks::use_panel_state::PanelState;
use crate::hooks::use_sse_state::SseState;

use super::types::{save_pinned_sessions, DeleteConfirm, RemoveProjectConfirm};

// ── Pin toggle ──────────────────────────────────────────────────────

pub fn build_toggle_pin(
    set_pinned_sessions: WriteSignal<HashSet<String>>,
) -> Callback<String> {
    Callback::new(move |session_id: String| {
        set_pinned_sessions.update(|pinned| {
            if pinned.contains(&session_id) {
                pinned.remove(&session_id);
            } else {
                pinned.insert(session_id);
            }
            save_pinned_sessions(pinned);
        });
    })
}

// ── Session selection ───────────────────────────────────────────────

pub fn build_select_session(
    _sse: SseState,
    panels: PanelState,
    mobile_open: ReadSignal<bool>,
    on_close: Callback<()>,
) -> Callback<(usize, String)> {
    Callback::new(move |(project_idx, session_id): (usize, String)| {
        // Auto-close sidebar on mobile
        if mobile_open.get_untracked() {
            on_close.run(());
        }
        // URL is the source of truth — update URL, which triggers session load
        crate::hooks::use_url_restore::navigate_to_session(
            &session_id,
            project_idx,
            &panels,
        );
    })
}

// ── New session ─────────────────────────────────────────────────────

/// New session targeting a specific project index.
pub fn build_new_session_for_project(sse: SseState) -> Callback<usize> {
    Callback::new(move |project_idx: usize| {
        sse.expect_session_switch();
        let set_app_state = sse.set_app_state;
        leptos::task::spawn_local(async move {
            #[derive(serde::Serialize)]
            struct NewBody { project_idx: usize }
            if let Err(e) = crate::api::api_post::<crate::types::api::NewSessionResponse>(
                "/session/new",
                &NewBody { project_idx },
            )
            .await
            {
                log::error!("Failed to create new session: {}", e);
                return;
            }
            if let Ok(state) = crate::api::project::fetch_app_state().await {
                set_app_state.set(Some(state));
            }
        });
    })
}

// ── Project switching ───────────────────────────────────────────────

pub fn build_switch_project(sse: SseState) -> Callback<usize> {
    Callback::new(move |project_idx: usize| {
        sse.expect_session_switch();
        let set_app_state = sse.set_app_state;
        leptos::task::spawn_local(async move {
            #[derive(serde::Serialize)]
            struct SwitchBody {
                index: usize,
            }
            if let Err(e) = crate::api::api_post_void(
                "/project/switch",
                &SwitchBody {
                    index: project_idx,
                },
            )
            .await
            {
                log::error!("Failed to switch project: {}", e);
                return;
            }
            // Fetch state as a reliable fallback.
            if let Ok(state) = crate::api::project::fetch_app_state().await {
                set_app_state.set(Some(state));
            }
        });
    })
}

// ── Delete session ──────────────────────────────────────────────────

pub fn build_do_delete(
    sse: SseState,
    set_delete_loading: WriteSignal<bool>,
    set_delete_confirm: WriteSignal<Option<DeleteConfirm>>,
) -> Callback<String> {
    Callback::new(move |session_id: String| {
        set_delete_loading.set(true);
        let set_app_state = sse.set_app_state;
        leptos::task::spawn_local(async move {
            let path = format!("/session/{}", js_sys::encode_uri_component(&session_id));
            if let Err(e) = crate::api::api_delete(&path).await {
                log::error!("Failed to delete session: {}", e);
            }
            // Fetch state as a reliable fallback.
            if let Ok(state) = crate::api::project::fetch_app_state().await {
                set_app_state.set(Some(state));
            }
            set_delete_loading.set(false);
            set_delete_confirm.set(None);
        });
    })
}

// ── Remove project ──────────────────────────────────────────────────

pub fn build_do_remove_project(
    sse: SseState,
    set_remove_project_loading: WriteSignal<bool>,
    set_remove_project_confirm: WriteSignal<Option<RemoveProjectConfirm>>,
) -> Callback<usize> {
    Callback::new(move |project_idx: usize| {
        set_remove_project_loading.set(true);
        let set_app_state = sse.set_app_state;
        leptos::task::spawn_local(async move {
            if let Err(e) = crate::api::project::remove_project(project_idx).await {
                log::error!("Failed to remove project: {}", e);
            }
            // Fetch state as a reliable fallback.
            if let Ok(state) = crate::api::project::fetch_app_state().await {
                set_app_state.set(Some(state));
            }
            set_remove_project_loading.set(false);
            set_remove_project_confirm.set(None);
        });
    })
}

// ── Rename session ──────────────────────────────────────────────────

pub fn build_rename_session(
    sse: SseState,
    set_renaming_sid: WriteSignal<Option<String>>,
    rename_original_title: ReadSignal<String>,
) -> Callback<(String, String)> {
    Callback::new(move |(session_id, new_title): (String, String)| {
        let trimmed = new_title.trim().to_string();
        // No-op guard: skip API call if title is empty or unchanged
        if trimmed.is_empty() || trimmed == rename_original_title.get_untracked() {
            set_renaming_sid.set(None);
            return;
        }
        let set_app_state = sse.set_app_state;
        leptos::task::spawn_local(async move {
            let path = format!("/session/{}", js_sys::encode_uri_component(&session_id));
            #[derive(serde::Serialize)]
            struct PatchBody {
                title: String,
            }
            if let Err(e) = crate::api::api_patch::<serde_json::Value>(
                &path,
                &PatchBody { title: trimmed },
            )
            .await
            {
                log::error!("Failed to rename session: {}", e);
            }
            // Fetch state so sidebar shows new title immediately.
            if let Ok(state) = crate::api::project::fetch_app_state().await {
                set_app_state.set(Some(state));
            }
            set_renaming_sid.set(None);
        });
    })
}
