//! ModalLayer — orchestrates rendering of all modal components.
//! Matches React `ModalLayer.tsx` — conditionally renders modals based on modal state.

use crate::hooks::use_modal_state::{ModalName, ModalState};
use crate::hooks::use_panel_state::PanelState;
use crate::hooks::use_sse_state::SseState;
use leptos::prelude::*;

use crate::components::add_project_modal::AddProjectModal;
use crate::components::agent_picker_modal::AgentPickerModal;
use crate::components::cheatsheet_modal::CheatsheetModal;
use crate::components::command_palette::CommandPalette;
use crate::components::context_input_modal::ContextInputModal;
use crate::components::model_picker_modal::ModelPickerModal;
use crate::components::session_selector_modal::SessionSelectorModal;
use crate::components::settings_modal::SettingsModal;
use crate::components::theme_selector_modal::{ThemeMode, ThemeSelectorModal};

// Phase 7 imports
use crate::components::activity_feed::ActivityFeed;
use crate::components::context_window_panel::ContextWindowPanel;
use crate::components::cross_session_search_modal::CrossSessionSearchModal;
use crate::components::diff_review_panel::DiffReviewPanel;
use crate::components::session_dashboard::SessionDashboard;
use crate::components::session_graph::SessionGraph;
use crate::components::split_view::SplitView;
use crate::components::todo_panel_modal::TodoPanelModal;

// Phase 8 imports
use crate::components::assistant_center_modal::AssistantCenterModal;
use crate::components::autonomy_modal::AutonomyModal;
use crate::components::delegation_board_modal::DelegationBoardModal;
use crate::components::inbox_modal::InboxModal;
use crate::components::memory_modal::MemoryModal;
use crate::components::missions_modal::MissionsModal;
use crate::components::notification_prefs_modal::NotificationPrefsModal;
use crate::components::routines_modal::RoutinesModal;
use crate::components::session_search_modal::SessionSearchModal;
use crate::components::system_monitor_modal::SystemMonitorModal;
use crate::components::watcher_modal::WatcherModal;
use crate::components::workspace_manager_modal::WorkspaceManagerModal;

use crate::hooks::use_assistant_state::AssistantState;

/// ModalLayer — renders all modals conditionally based on modal state.
#[component]
pub fn ModalLayer(
    sse: SseState,
    modal_state: ModalState,
    panels: PanelState,
    on_command: Callback<(String, Option<String>)>,
    on_new_session: Callback<()>,
    on_select_session: Callback<(String, usize)>,
    on_model_selected: Callback<(String, String)>,
    on_agent_change: Callback<String>,
    on_context_submit: Callback<String>,
    on_theme_applied: Callback<crate::types::api::ThemeColors>,
    theme_mode: ReadSignal<ThemeMode>,
    set_theme_mode: WriteSignal<ThemeMode>,
) -> impl IntoView {
    let close = move |name: ModalName| {
        Callback::new(move |_: ()| {
            modal_state.close(name);
        })
    };

    // Derive active session ID from narrow derived_active_project (avoids subscribing to full app_state)
    let active_session_id_memo = Memo::new(move |_| {
        sse.derived_active_project
            .get()
            .and_then(|p| p.active_session.clone())
    });
    let active_session_id = move || active_session_id_memo.get();

    // Current agent from app state (simplified)
    let current_agent = move || {
        // Default to "coder" — the real value would come from session state
        "coder".to_string()
    };

    // ── Phase 7 helpers ────────────────────────────────────────────────

    // Active project index (narrow: derived from derived_active_project, not full app_state)
    let active_project_idx_memo = Memo::new(move |_| {
        sse.app_state
            .with_untracked(|s| s.as_ref().map(|s| s.active_project).unwrap_or(0))
    });
    let active_project_idx = move || active_project_idx_memo.get();

    // File edit count — read from SSE state (incremented by file.edited events)
    let file_edit_count = sse.file_edit_count;

    // Adapter: on_select_session expects (String, usize) but
    // SessionGraph/SessionDashboard emit (usize, String)
    let on_select_session_idx_id = {
        let cb = on_select_session;
        Callback::new(move |(idx, id): (usize, String)| {
            cb.run((id, idx));
        })
    };

    // Adapter: CrossSessionSearch on_navigate emits just a String session id.
    // We select it in the active project (index 0 / active_project).
    let on_select_session_id_only = {
        let cb = on_select_session;
        Callback::new(move |id: String| {
            let idx = sse
                .app_state
                .get_untracked()
                .map(|s| s.active_project)
                .unwrap_or(0);
            cb.run((id, idx));
        })
    };

    view! {
        // Command Palette
        {move || {
            if modal_state.is_open_tracked(ModalName::CommandPalette) {
                let sid = active_session_id();
                Some(view! {
                    <CommandPalette
                        on_close=close(ModalName::CommandPalette)
                        on_command=on_command
                        on_new_session=on_new_session
                        modal_state=modal_state
                        panels=panels
                        session_id=sid
                    />
                })
            } else {
                None
            }
        }}

        // Model Picker
        {move || {
            if modal_state.is_open_tracked(ModalName::ModelPicker) {
                let sid = active_session_id();
                Some(view! {
                    <ModelPickerModal
                        on_close=close(ModalName::ModelPicker)
                        session_id=sid
                        on_model_selected=on_model_selected
                    />
                })
            } else {
                None
            }
        }}

        // Agent Picker
        {move || {
            if modal_state.is_open_tracked(ModalName::AgentPicker) {
                let agent = current_agent();
                Some(view! {
                    <AgentPickerModal
                        on_close=close(ModalName::AgentPicker)
                        current_agent=agent
                        on_agent_selected=on_agent_change
                    />
                })
            } else {
                None
            }
        }}

        // Theme Selector
        {move || {
            if modal_state.is_open_tracked(ModalName::ThemeSelector) {
                Some(view! {
                    <ThemeSelectorModal
                        on_close=close(ModalName::ThemeSelector)
                        on_theme_applied=on_theme_applied
                        theme_mode=theme_mode
                        set_theme_mode=set_theme_mode
                    />
                })
            } else {
                None
            }
        }}

        // Cheatsheet
        {move || {
            if modal_state.is_open_tracked(ModalName::Cheatsheet) {
                Some(view! {
                    <CheatsheetModal on_close=close(ModalName::Cheatsheet) />
                })
            } else {
                None
            }
        }}

        // Session Selector
        {move || {
            if modal_state.is_open_tracked(ModalName::SessionSelector) {
                let projects = sse.app_state.get_untracked()
                    .map(|s| s.projects.clone())
                    .unwrap_or_default();
                let sid = active_session_id();
                Some(view! {
                    <SessionSelectorModal
                        on_close=close(ModalName::SessionSelector)
                        projects=projects
                        active_session_id=sid
                        on_select_session=on_select_session
                    />
                })
            } else {
                None
            }
        }}

        // Context Input
        {move || {
            if modal_state.is_open_tracked(ModalName::ContextInput) {
                Some(view! {
                    <ContextInputModal
                        on_close=close(ModalName::ContextInput)
                        on_submit=on_context_submit
                    />
                })
            } else {
                None
            }
        }}

        // Settings
        {move || {
            if modal_state.is_open_tracked(ModalName::Settings) {
                Some(view! {
                    <SettingsModal
                        on_close=close(ModalName::Settings)
                        modal_state=modal_state
                        panels=panels
                    />
                })
            } else {
                None
            }
        }}

        // Add Project
        {move || {
            if modal_state.is_open_tracked(ModalName::AddProject) {
                Some(view! {
                    <AddProjectModal on_close=close(ModalName::AddProject) />
                })
            } else {
                None
            }
        }}

        // ── Phase 7: Feature modals / panels ────────────────────────────

        // Todo Panel
        {move || {
            if modal_state.is_open_tracked(ModalName::TodoPanel) {
                let sid = active_session_id().unwrap_or_default();
                Some(view! {
                    <TodoPanelModal
                        on_close=close(ModalName::TodoPanel)
                        session_id=sid
                    />
                })
            } else {
                None
            }
        }}

        // Context Window
        {move || {
            if modal_state.is_open_tracked(ModalName::ContextWindow) {
                let sid = active_session_id();
                Some(view! {
                    <ContextWindowPanel
                        on_close=close(ModalName::ContextWindow)
                        session_id=sid
                        on_compact=close(ModalName::ContextWindow)
                    />
                })
            } else {
                None
            }
        }}

        // Diff Review
        {move || {
            if modal_state.is_open_tracked(ModalName::DiffReview) {
                let sid = active_session_id();
                Some(view! {
                    <DiffReviewPanel
                        on_close=close(ModalName::DiffReview)
                        session_id=sid
                        file_edit_count=file_edit_count
                    />
                })
            } else {
                None
            }
        }}

        // Cross-Session Search
        {move || {
            if modal_state.is_open_tracked(ModalName::CrossSearch) {
                let pidx = active_project_idx();
                Some(view! {
                    <CrossSessionSearchModal
                        on_close=close(ModalName::CrossSearch)
                        project_idx=pidx
                        on_navigate=on_select_session_id_only
                    />
                })
            } else {
                None
            }
        }}

        // Split View
        {move || {
            if modal_state.is_open_tracked(ModalName::SplitView) {
                let sid = active_session_id();
                let sessions = sse.app_state.get_untracked()
                    .map(|s| {
                        let idx = s.active_project;
                        s.projects.get(idx).map(|p| p.sessions.clone()).unwrap_or_default()
                    })
                    .unwrap_or_default();
                Some(view! {
                    <SplitView
                        primary_session_id=sid
                        secondary_session_id=modal_state.split_view_secondary_id
                        set_secondary_session_id=modal_state.set_split_view_secondary_id
                        on_close=close(ModalName::SplitView)
                        sessions=sessions
                    />
                })
            } else {
                None
            }
        }}

        // Session Graph
        {move || {
            if modal_state.is_open_tracked(ModalName::SessionGraph) {
                let sid = active_session_id();
                Some(view! {
                    <SessionGraph
                        on_select_session=on_select_session_idx_id
                        on_close=close(ModalName::SessionGraph)
                        active_session_id=sid
                    />
                })
            } else {
                None
            }
        }}

        // Session Dashboard
        {move || {
            if modal_state.is_open_tracked(ModalName::SessionDashboard) {
                let sid = active_session_id();
                Some(view! {
                    <SessionDashboard
                        on_select_session=on_select_session_idx_id
                        on_close=close(ModalName::SessionDashboard)
                        active_session_id=sid
                    />
                })
            } else {
                None
            }
        }}

        // Activity Feed
        {move || {
            if modal_state.is_open_tracked(ModalName::ActivityFeed) {
                let sid = active_session_id();
                Some(view! {
                    <ActivityFeed
                        session_id=sid
                        on_close=close(ModalName::ActivityFeed)
                    />
                })
            } else {
                None
            }
        }}

        // ── Phase 8: Assistant modals ───────────────────────────────────

        // Autonomy
        {move || {
            if modal_state.is_open_tracked(ModalName::Autonomy) {
                // Read real autonomy mode from AssistantState context
                let assistant = use_context::<AssistantState>();
                let mode = assistant
                    .map(|a| a.autonomy_mode.get_untracked())
                    .unwrap_or_else(|| "observe".to_string());
                Some(view! {
                    <AutonomyModal
                        on_close=close(ModalName::Autonomy)
                        mode=mode
                        on_change=Callback::new(move |new_mode: String| {
                            // Optimistic update local state
                            if let Some(a) = assistant {
                                a.set_autonomy_mode.set(new_mode.clone());
                            }
                            // Fire-and-forget POST to backend (matches React: updateAutonomySettings)
                            leptos::task::spawn_local(async move {
                                let _ = crate::api::missions::update_autonomy_settings(&new_mode).await;
                            });
                            modal_state.close(ModalName::Autonomy);
                        })
                    />
                })
            } else {
                None
            }
        }}

        // Notification Prefs
        {move || {
            if modal_state.is_open_tracked(ModalName::NotificationPrefs) {
                Some(view! {
                    <NotificationPrefsModal on_close=close(ModalName::NotificationPrefs) />
                })
            } else {
                None
            }
        }}

        // Delegation Board
        {move || {
            if modal_state.is_open_tracked(ModalName::Delegation) {
                let sid = active_session_id();
                Some(view! {
                    <DelegationBoardModal
                        on_close=close(ModalName::Delegation)
                        missions=vec![]
                        active_session_id=sid
                    />
                })
            } else {
                None
            }
        }}

        // Memory
        {move || {
            if modal_state.is_open_tracked(ModalName::Memory) {
                let sid = active_session_id();
                let pidx = active_project_idx();
                let projects = sse.app_state.get_untracked()
                    .map(|s| s.projects.clone())
                    .unwrap_or_default();
                Some(view! {
                    <MemoryModal
                        on_close=close(ModalName::Memory)
                        projects=projects
                        active_project_index=pidx
                        active_session_id=sid
                    />
                })
            } else {
                None
            }
        }}

        // Missions
        {move || {
            if modal_state.is_open_tracked(ModalName::Missions) {
                let sid = active_session_id();
                let pidx = active_project_idx();
                let projects = sse.app_state.get_untracked()
                    .map(|s| s.projects.clone())
                    .unwrap_or_default();
                Some(view! {
                    <MissionsModal
                        on_close=close(ModalName::Missions)
                        projects=projects
                        active_project_index=pidx
                        active_session_id=sid
                    />
                })
            } else {
                None
            }
        }}

        // Inbox
        {move || {
            if modal_state.is_open_tracked(ModalName::Inbox) {
                Some(view! {
                    <InboxModal
                        on_close=close(ModalName::Inbox)
                        on_open_missions=Callback::new(move |_: ()| {
                            modal_state.close(ModalName::Inbox);
                            modal_state.open(ModalName::Missions);
                        })
                    />
                })
            } else {
                None
            }
        }}

        // Workspace Manager
        {move || {
            if modal_state.is_open_tracked(ModalName::WorkspaceManager) {
                // Build current snapshot matching React's buildCurrentSnapshot
                let assistant = use_context::<AssistantState>();
                let active_ws_name = assistant.and_then(|a| a.active_workspace_name.get());
                let build_snapshot = Callback::new(move |_: ()| {
                    let sid = active_session_id();
                    crate::types::api::WorkspaceSnapshot {
                        name: String::new(),
                        created_at: js_sys::Date::new_0()
                            .to_iso_string()
                            .as_string()
                            .unwrap_or_default(),
                        panels: crate::types::api::WorkspacePanels {
                            sidebar: panels.sidebar_open.get_untracked(),
                            terminal: panels.terminal.open.get_untracked(),
                            editor: panels.editor.open.get_untracked(),
                            git: panels.git.open.get_untracked(),
                        },
                        layout: crate::types::api::WorkspaceLayout {
                            sidebar_width: 0.0,
                            terminal_height: 0.0,
                            side_panel_width: 0.0,
                        },
                        open_files: Vec::new(),
                        active_file: None,
                        terminal_tabs: Vec::new(),
                        session_id: sid,
                        git_branch: None,
                        is_template: false,
                        recipe_description: None,
                        recipe_next_action: None,
                        is_recipe: None,
                    }
                });
                Some(view! {
                    <WorkspaceManagerModal
                        on_close=close(ModalName::WorkspaceManager)
                        on_restore=Callback::new(move |_snap: crate::types::api::WorkspaceSnapshot| {
                            // TODO: apply workspace snapshot
                            modal_state.close(ModalName::WorkspaceManager);
                        })
                        on_save_current=build_snapshot
                        active_workspace_name=active_ws_name.unwrap_or_default()
                    />
                })
            } else {
                None
            }
        }}

        // Routines
        {move || {
            if modal_state.is_open_tracked(ModalName::Routines) {
                let sid = active_session_id();
                match sid {
                    Some(s) => Some(view! {
                        <RoutinesModal
                            on_close=close(ModalName::Routines)
                            active_session_id=s
                        />
                    }.into_any()),
                    None => Some(view! {
                        <RoutinesModal
                            on_close=close(ModalName::Routines)
                        />
                    }.into_any()),
                }
            } else {
                None
            }
        }}

        // Watcher
        {move || {
            if modal_state.is_open_tracked(ModalName::Watcher) {
                Some(view! {
                    <WatcherModal
                        on_close=close(ModalName::Watcher)
                    />
                })
            } else {
                None
            }
        }}

        // System Monitor
        {move || {
            if modal_state.is_open_tracked(ModalName::SystemMonitor) {
                Some(view! {
                    <SystemMonitorModal on_close=close(ModalName::SystemMonitor) />
                })
            } else {
                None
            }
        }}

        // Assistant Center
        {move || {
            if modal_state.is_open_tracked(ModalName::AssistantCenter) {
                Some(view! {
                    <AssistantCenterModal
                        on_close=close(ModalName::AssistantCenter)
                        on_open_inbox=Callback::new(move |_: ()| {
                            modal_state.close(ModalName::AssistantCenter);
                            modal_state.open(ModalName::Inbox);
                        })
                        on_open_missions=Callback::new(move |_: ()| {
                            modal_state.close(ModalName::AssistantCenter);
                            modal_state.open(ModalName::Missions);
                        })
                        on_open_memory=Callback::new(move |_: ()| {
                            modal_state.close(ModalName::AssistantCenter);
                            modal_state.open(ModalName::Memory);
                        })
                        on_open_autonomy=Callback::new(move |_: ()| {
                            modal_state.close(ModalName::AssistantCenter);
                            modal_state.open(ModalName::Autonomy);
                        })
                        on_open_routines=Callback::new(move |_: ()| {
                            modal_state.close(ModalName::AssistantCenter);
                            modal_state.open(ModalName::Routines);
                        })
                        on_open_delegation=Callback::new(move |_: ()| {
                            modal_state.close(ModalName::AssistantCenter);
                            modal_state.open(ModalName::Delegation);
                        })
                        on_open_workspaces=Callback::new(move |_: ()| {
                            modal_state.close(ModalName::AssistantCenter);
                            modal_state.open(ModalName::WorkspaceManager);
                        })
                    />
                })
            } else {
                None
            }
        }}

        // Session Search
        {move || {
            if modal_state.is_open_tracked(ModalName::SessionSearch) {
                let state = sse.app_state.get_untracked();
                if let Some(state) = state {
                    let pidx = state.active_project;
                    if let Some(project) = state.projects.get(pidx).cloned() {
                        Some(view! {
                            <SessionSearchModal
                                project=project
                                project_idx=pidx
                                on_select=on_select_session_idx_id
                                on_close=close(ModalName::SessionSearch)
                            />
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }}
    }
}
