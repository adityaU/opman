//! Assistant state hook — memory, autonomy, missions, routines, delegation, workspaces, pulse.
//! Matches React `useAssistantState.ts`.

use leptos::prelude::*;

use crate::api::client::{api_fetch, api_post};
use crate::types::api::{
    AutonomySettings, DelegatedWorkItem, DelegatedWorkListResponse, Mission,
    MissionsListResponse, PersonalMemoryItem, PersonalMemoryListResponse,
    RoutineDefinition, RoutineRunRecord, RoutinesListResponse,
    WorkspaceSnapshot, WorkspacesListResponse,
    AssistantRecommendation, RecommendationsResponse,
};

// ── Types ──────────────────────────────────────────────────────────

/// Assistant state returned by `use_assistant_state`.
#[derive(Clone, Copy)]
pub struct AssistantState {
    pub personal_memory: ReadSignal<Vec<PersonalMemoryItem>>,
    pub set_personal_memory: WriteSignal<Vec<PersonalMemoryItem>>,

    pub autonomy_mode: ReadSignal<String>,
    pub set_autonomy_mode: WriteSignal<String>,

    pub mission_cache: ReadSignal<Vec<Mission>>,
    pub set_mission_cache: WriteSignal<Vec<Mission>>,

    pub routine_cache: ReadSignal<Vec<RoutineDefinition>>,
    pub set_routine_cache: WriteSignal<Vec<RoutineDefinition>>,

    pub routine_run_cache: ReadSignal<Vec<RoutineRunRecord>>,
    pub set_routine_run_cache: WriteSignal<Vec<RoutineRunRecord>>,

    pub delegated_work_cache: ReadSignal<Vec<DelegatedWorkItem>>,
    pub set_delegated_work_cache: WriteSignal<Vec<DelegatedWorkItem>>,

    pub workspace_cache: ReadSignal<Vec<WorkspaceSnapshot>>,
    pub set_workspace_cache: WriteSignal<Vec<WorkspaceSnapshot>>,

    pub active_memory_items: ReadSignal<Vec<PersonalMemoryItem>>,
    pub set_active_memory_items: WriteSignal<Vec<PersonalMemoryItem>>,

    pub assistant_pulse: ReadSignal<Option<AssistantRecommendation>>,
    pub set_assistant_pulse: WriteSignal<Option<AssistantRecommendation>>,

    pub latest_daily_summary: ReadSignal<Option<String>>,
    pub set_latest_daily_summary: WriteSignal<Option<String>>,

    pub active_workspace_name: ReadSignal<Option<String>>,
    pub set_active_workspace_name: WriteSignal<Option<String>>,

    /// Trigger refresh for routines.
    pub refresh_routines_trigger: WriteSignal<u32>,
}

impl AssistantState {
    /// Force a refresh of routines from the server.
    pub fn refresh_routines(&self) {
        self.refresh_routines_trigger.update(|v| *v += 1);
    }
}

// ── Hook ───────────────────────────────────────────────────────────

/// Create assistant state. Call once at layout level.
pub fn use_assistant_state(
    app_state: ReadSignal<Option<crate::types::api::AppState>>,
    active_session_id_fn: impl Fn() -> Option<String> + 'static + Copy,
) -> AssistantState {
    let (personal_memory, set_personal_memory) = signal(Vec::<PersonalMemoryItem>::new());
    let (autonomy_mode, set_autonomy_mode) = signal("observe".to_string());
    let (mission_cache, set_mission_cache) = signal(Vec::<Mission>::new());
    let (routine_cache, set_routine_cache) = signal(Vec::<RoutineDefinition>::new());
    let (routine_run_cache, set_routine_run_cache) = signal(Vec::<RoutineRunRecord>::new());
    let (delegated_work_cache, set_delegated_work_cache) = signal(Vec::<DelegatedWorkItem>::new());
    let (workspace_cache, set_workspace_cache) = signal(Vec::<WorkspaceSnapshot>::new());
    let (active_memory_items, set_active_memory_items) = signal(Vec::<PersonalMemoryItem>::new());
    let (assistant_pulse, set_assistant_pulse) = signal(Option::<AssistantRecommendation>::None);
    let (latest_daily_summary, set_latest_daily_summary) = signal(Option::<String>::None);
    let (active_workspace_name, set_active_workspace_name) = signal(Option::<String>::None);

    let (refresh_routines_trigger, set_refresh_routines_trigger) = signal(0u32);

    // Fetch personal memory on mount
    {
        let set_mem = set_personal_memory;
        Effect::new(move |_| {
            leptos::task::spawn_local(async move {
                if let Ok(resp) = api_fetch::<PersonalMemoryListResponse>("/assistant/memory").await
                {
                    set_mem.set(resp.memory);
                }
            });
        });
    }

    // Fetch autonomy settings on mount
    {
        let set_mode = set_autonomy_mode;
        Effect::new(move |_| {
            leptos::task::spawn_local(async move {
                if let Ok(resp) = api_fetch::<AutonomySettings>("/assistant/autonomy").await {
                    set_mode.set(resp.mode);
                }
            });
        });
    }

    // Fetch routines on mount and when trigger changes
    {
        let set_routines = set_routine_cache;
        let set_runs = set_routine_run_cache;
        Effect::new(move |_| {
            let _trigger = refresh_routines_trigger.get();
            leptos::task::spawn_local(async move {
                if let Ok(resp) = api_fetch::<RoutinesListResponse>("/assistant/routines").await {
                    set_routines.set(resp.routines);
                    set_runs.set(resp.runs);
                }
            });
        });
    }

    // Fetch missions on mount
    {
        let set_missions = set_mission_cache;
        Effect::new(move |_| {
            leptos::task::spawn_local(async move {
                if let Ok(resp) = api_fetch::<MissionsListResponse>("/assistant/missions").await {
                    set_missions.set(resp.missions);
                }
            });
        });
    }

    // Fetch delegated work on mount
    {
        let set_del = set_delegated_work_cache;
        Effect::new(move |_| {
            leptos::task::spawn_local(async move {
                if let Ok(resp) =
                    api_fetch::<DelegatedWorkListResponse>("/assistant/delegation").await
                {
                    set_del.set(resp.items);
                }
            });
        });
    }

    // Fetch workspaces on mount
    {
        let set_ws = set_workspace_cache;
        Effect::new(move |_| {
            leptos::task::spawn_local(async move {
                if let Ok(resp) =
                    api_fetch::<WorkspacesListResponse>("/assistant/workspaces").await
                {
                    set_ws.set(resp.workspaces);
                }
            });
        });
    }

    // Fetch active memory items (backend-driven, depends on project + session)
    {
        let set_active = set_active_memory_items;
        // Use a narrow derived memo for just the active project index,
        // so this effect doesn't re-run on every app_state mutation
        // (e.g. session title/time updates).
        let active_proj_idx = Memo::new(move |_| {
            app_state.get().map(|s| s.active_project).unwrap_or(0)
        });
        Effect::new(move |_| {
            let project_idx = active_proj_idx.get(); // track only project index changes
            let sid = active_session_id_fn();

            leptos::task::spawn_local(async move {
                let mut path = format!("/memory/active?project_index={}", project_idx);
                if let Some(ref s) = sid {
                    path.push_str(&format!(
                        "&session_id={}",
                        js_sys::encode_uri_component(s)
                    ));
                }
                if let Ok(resp) = api_fetch::<PersonalMemoryListResponse>(&path).await {
                    set_active.set(resp.memory);
                }
            });
        });
    }

    // Compute recommendations (pulse)
    {
        let set_pulse = set_assistant_pulse;
        Effect::new(move |_| {
            // Track deps that should cause re-computation
            let _mode = autonomy_mode.get();
            let _missions = mission_cache.get();
            let _routines = routine_cache.get();

            leptos::task::spawn_local(async move {
                if let Ok(resp) =
                    api_post::<RecommendationsResponse>(
                        "/assistant/recommendations",
                        &serde_json::json!({}),
                    )
                    .await
                {
                    set_pulse.set(resp.recommendations.into_iter().next());
                }
            });
        });
    }

    // Derive latest daily summary from routine run cache
    {
        let set_summary = set_latest_daily_summary;
        Effect::new(move |_| {
            let routines = routine_cache.get();
            let runs = routine_run_cache.get();

            let daily_ids: std::collections::HashSet<String> = routines
                .iter()
                .filter(|r| r.trigger == "daily_summary")
                .map(|r| r.id.clone())
                .collect();

            let latest = runs
                .iter()
                .find(|r| daily_ids.contains(&r.routine_id));

            if let Some(run) = latest {
                set_summary.set(Some(run.summary.clone()));
            }
        });
    }

    AssistantState {
        personal_memory,
        set_personal_memory,
        autonomy_mode,
        set_autonomy_mode,
        mission_cache,
        set_mission_cache,
        routine_cache,
        set_routine_cache,
        routine_run_cache,
        set_routine_run_cache,
        delegated_work_cache,
        set_delegated_work_cache,
        workspace_cache,
        set_workspace_cache,
        active_memory_items,
        set_active_memory_items,
        assistant_pulse,
        set_assistant_pulse,
        latest_daily_summary,
        set_latest_daily_summary,
        active_workspace_name,
        set_active_workspace_name,
        refresh_routines_trigger: set_refresh_routines_trigger,
    }
}
