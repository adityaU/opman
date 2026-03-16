//! Pulse actions hook — dispatches the top assistant recommendation action.
//! Matches React `usePulseActions.ts`.

use leptos::prelude::*;

use crate::api::client::{api_fetch, api_post, api_post_void};
use crate::hooks::use_modal_state::ModalState;
use crate::hooks::use_toast::{ToastExt, ToastState};
use crate::types::api::{
    AssistantRecommendation, RoutineDefinition,
    WorkspaceSnapshot, WorkspacesListResponse,
};

/// Pulse actions returned by `use_pulse_actions`.
#[derive(Clone)]
pub struct PulseActions {
    pub handle_run_assistant_pulse: Callback<()>,
}

/// Create pulse action handler. Call once at layout level.
pub fn use_pulse_actions(
    assistant_pulse: ReadSignal<Option<AssistantRecommendation>>,
    active_session_id_fn: impl Fn() -> Option<String> + Send + Sync + Copy + 'static,
    modals: ModalState,
    toasts: ToastState,
    set_autonomy_mode: WriteSignal<String>,
    set_routine_cache: WriteSignal<Vec<RoutineDefinition>>,
    set_workspace_cache: WriteSignal<Vec<WorkspaceSnapshot>>,
) -> PulseActions {
    let handle_run_assistant_pulse = Callback::new(move |()| {
        let pulse = assistant_pulse.get_untracked();
        let pulse = match pulse {
            Some(p) => p,
            None => return,
        };

        match pulse.action.as_str() {
            "open_inbox" => modals.open_str("inbox"),
            "open_missions" => modals.open_str("missions"),
            "open_memory" => modals.open_str("memory"),
            "open_routines" => modals.open_str("routines"),
            "open_delegation" => modals.open_str("delegation"),
            "open_workspaces" => modals.open_str("workspaceManager"),
            "open_autonomy" => modals.open_str("autonomy"),

            "setup_daily_summary" => {
                leptos::task::spawn_local(async move {
                    match api_post::<RoutineDefinition>(
                        "/assistant/routines",
                        &serde_json::json!({
                            "name": "Daily Briefing",
                            "trigger": "daily_summary",
                            "action": "open_inbox",
                        }),
                    )
                    .await
                    {
                        Ok(routine) => {
                            set_routine_cache.update(|v| v.insert(0, routine));
                            toasts.add_typed("Daily briefing enabled", "success");
                        }
                        Err(_) => toasts.add_typed("Failed to enable daily briefing", "error"),
                    }
                });
            }

            "upgrade_autonomy_nudge" => {
                set_autonomy_mode.set("nudge".to_string());
                leptos::task::spawn_local(async move {
                    match api_post_void(
                        "/assistant/autonomy",
                        &serde_json::json!({ "mode": "nudge" }),
                    )
                    .await
                    {
                         Ok(()) => toasts.add_typed("Autonomy set to Nudge", "success"),
                        Err(_) => toasts.add_typed("Failed to update autonomy", "error"),
                    }
                });
            }

            "setup_daily_copilot" => {
                let sid = active_session_id_fn();
                leptos::task::spawn_local(async move {
                    // Create routine
                    let routine_result = api_post::<RoutineDefinition>(
                        "/assistant/routines",
                        &serde_json::json!({
                            "name": "Daily Briefing",
                            "trigger": "daily_summary",
                            "action": "open_inbox",
                        }),
                    )
                    .await;

                    if let Ok(routine) = routine_result {
                        set_routine_cache.update(|v| {
                            if !v.iter().any(|r| r.name == routine.name) {
                                v.insert(0, routine);
                            }
                        });
                    }

                    // Update autonomy
                    if api_post_void(
                        "/assistant/autonomy",
                        &serde_json::json!({ "mode": "nudge" }),
                    )
                    .await
                    .is_ok()
                    {
                        set_autonomy_mode.set("nudge".to_string());
                    }

                    // Save workspace
                    let _ = api_post_void(
                        "/assistant/workspaces",
                        &serde_json::json!({
                            "name": "Morning Review",
                            "panels": { "sidebar": true, "terminal": false, "editor": false, "git": true },
                            "layout": { "sidebar_width": 320, "terminal_height": 0, "side_panel_width": 480 },
                            "open_files": [],
                            "active_file": null,
                            "terminal_tabs": [],
                            "session_id": sid,
                            "is_template": false,
                            "is_recipe": true,
                            "recipe_description": "Start the day with missions, inbox, and git context ready.",
                            "recipe_next_action": "Review the assistant summary, clear blockers, then choose the next mission.",
                        }),
                    )
                    .await;

                    // Refresh workspaces
                    if let Ok(resp) =
                        api_fetch::<WorkspacesListResponse>("/assistant/workspaces").await
                    {
                        set_workspace_cache.set(resp.workspaces);
                    }

                    toasts.add_typed("Daily Copilot preset enabled", "success");
                });
            }
            _ => {}
        }
    });

    PulseActions {
        handle_run_assistant_pulse,
    }
}
