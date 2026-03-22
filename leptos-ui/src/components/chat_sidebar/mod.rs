//! ChatSidebar — project tree and session list.
//! Matches React `ChatSidebar.tsx` + `ProjectNode.tsx` + `ContextMenu.tsx` + `ConfirmModals.tsx`.

mod callbacks;
mod overlays;
mod project_node;
mod project_sessions;
mod session_button;
mod session_row;
mod subagent_list;
pub mod types;

use leptos::prelude::*;

use crate::components::icons::*;
use crate::hooks::use_modal_state::{ModalName, ModalState};
use crate::hooks::use_sse_state::SseState;

use callbacks::*;
use overlays::*;
use project_node::ProjectNode;
use types::*;

// ── ChatSidebar component ───────────────────────────────────────────

#[component]
pub fn ChatSidebar(
    sse: SseState,
    modal_state: ModalState,
    mobile_open: ReadSignal<bool>,
    on_close: Callback<()>,
) -> impl IntoView {
    let active_session_id = Memo::new(move |_| sse.tracked_session_id_reactive());

    // Use derived signals to avoid subscribing to the full monolithic app_state.
    let projects = sse.derived_projects;
    let active_project_idx = sse.derived_active_project_idx;

    // UI state
    let (expanded_project, set_expanded_project) = signal::<Option<usize>>(None);
    let (expanded_subagents, set_expanded_subagents) = signal::<Option<String>>(None);
    let (show_more_project, set_show_more_project) = signal::<Option<usize>>(None);
    let (search_visible, set_search_visible) = signal(false);
    let (search_query, set_search_query) = signal(String::new());

    // Pinned sessions
    let (pinned_sessions, set_pinned_sessions) = signal(load_pinned_sessions());

    // Context menu
    let (ctx_menu, set_ctx_menu) = signal::<Option<ContextMenuState>>(None);

    // Rename state
    let (renaming_sid, set_renaming_sid) = signal::<Option<String>>(None);
    let (rename_text, set_rename_text) = signal(String::new());
    let (rename_original_title, set_rename_original_title) = signal(String::new());
    let rename_input_ref = NodeRef::<leptos::html::Input>::new();

    // Delete confirmation
    let (delete_confirm, set_delete_confirm) = signal::<Option<DeleteConfirm>>(None);
    let (delete_loading, set_delete_loading) = signal(false);

    // Remove project confirmation
    let (remove_project_confirm, set_remove_project_confirm) =
        signal::<Option<RemoveProjectConfirm>>(None);
    let (remove_project_loading, set_remove_project_loading) = signal(false);

    // ── Effects ─────────────────────────────────────────────────────

    // Auto-expand active project on first load
    Effect::new(move |prev_expanded: Option<bool>| {
        let idx = active_project_idx.get();
        if prev_expanded.is_none() {
            set_expanded_project.set(Some(idx));
        }
        true
    });

    // Close context menu on document click or Escape
    setup_ctx_menu_dismiss(ctx_menu, set_ctx_menu);

    // Focus + select-all rename input when renaming starts
    Effect::new(move |_| {
        let sid = renaming_sid.get();
        if sid.is_some() {
            leptos::task::spawn_local(async move {
                gloo_timers::future::sleep(std::time::Duration::from_millis(10)).await;
                if let Some(input) = rename_input_ref.get() {
                    let _ = input.focus();
                    let _ = input.select();
                }
            });
        }
    });

    // ── Callbacks ───────────────────────────────────────────────────

    let toggle_pin = build_toggle_pin(set_pinned_sessions);
    let panels = expect_context::<crate::hooks::use_panel_state::PanelState>();
    let select_session = build_select_session(sse, panels, mobile_open, on_close);
    let new_session_for_project = build_new_session_for_project(sse);
    let rename_session = build_rename_session(sse, set_renaming_sid, rename_original_title);
    let do_delete = build_do_delete(sse, set_delete_loading, set_delete_confirm);
    let do_remove_project =
        build_do_remove_project(sse, set_remove_project_loading, set_remove_project_confirm);

    let toggle_project_expand = Callback::new(move |index: usize| {
        let current = expanded_project.get_untracked();
        if current == Some(index) {
            set_expanded_project.set(None);
        } else {
            set_expanded_project.set(Some(index));
        }
        set_expanded_subagents.set(None);
        set_show_more_project.set(None);
    });

    view! {
        // Mobile overlay backdrop
        {move || {
            if mobile_open.get() {
                Some(view! {
                    <div
                        class="sidebar-mobile-overlay"
                        on:click=move |_| on_close.run(())
                        aria-hidden="true"
                    />
                })
            } else {
                None
            }
        }}

        <aside class=move || {
            let base = "chat-sidebar";
            if mobile_open.get() { format!("{} mobile-open", base) } else { base.to_string() }
        }>
            // Header
            <div class="sb-header">
                <span class="sb-brand">"Sessions"</span>
                <div class="sb-header-actions">
                    <button
                        class="sb-icon-btn"
                        on:click=move |_| set_search_visible.update(|v| *v = !*v)
                        title="Search sessions"
                        aria-label="Search sessions"
                    >
                        <IconSearch size=14 />
                    </button>
                    <button
                        class="sidebar-close-btn"
                        on:click=move |_| on_close.run(())
                        aria-label="Close sidebar"
                    >
                        <IconX size=14 />
                    </button>
                </div>
            </div>

            // Search bar (collapsible)
            {move || {
                if search_visible.get() {
                    Some(view! {
                        <div class="sb-search">
                            <IconSearch size=12 class="sb-search-icon" />
                            <input
                                class="sb-search-input"
                                type="text"
                                placeholder="Filter sessions..."
                                autofocus=true
                                prop:value=search_query
                                on:input=move |ev| set_search_query.set(event_target_value(&ev))
                            />
                            {move || {
                                if !search_query.get().is_empty() {
                                    Some(view! {
                                        <button
                                            class="sb-search-clear"
                                            on:click=move |_| set_search_query.set(String::new())
                                            aria-label="Clear search"
                                        >
                                            <IconX size=10 />
                                        </button>
                                    })
                                } else {
                                    None
                                }
                            }}
                        </div>
                    })
                } else {
                    None
                }
            }}

            // Project list
            <div class="sb-list" on:click=move |_| set_ctx_menu.set(None)>
                <For
                    each=move || {
                        projects.get().into_iter().enumerate().collect::<Vec<_>>()
                    }
                    key=|(idx, p)| {
                        // Include session fingerprint so ProjectNode rebuilds
                        // when sessions are added, removed, renamed, or reordered.
                        use std::hash::{Hash, Hasher};
                        let mut h = std::hash::DefaultHasher::new();
                        idx.hash(&mut h);
                        p.path.hash(&mut h);
                        p.sessions.len().hash(&mut h);
                        for s in &p.sessions {
                            s.id.hash(&mut h);
                            s.title.hash(&mut h);
                        }
                        h.finish()
                    }
                    children=move |(idx, project)| {
                        view! {
                            <ProjectNode
                                idx=idx
                                project_name=project.name.clone()
                                git_branch=project.git_branch.clone()
                                sessions=project.sessions.clone()
                                sse=sse
                                active_project_idx=active_project_idx
                                active_session_id=active_session_id
                                expanded_project=expanded_project
                                pinned_sessions=pinned_sessions
                                expanded_subagents=expanded_subagents
                                set_expanded_subagents=set_expanded_subagents
                                show_more_project=show_more_project
                                set_show_more_project=set_show_more_project
                                search_query=search_query
                                renaming_sid=renaming_sid
                                set_renaming_sid=set_renaming_sid
                                rename_text=rename_text
                                set_rename_text=set_rename_text
                                rename_original_title=rename_original_title
                                set_rename_original_title=set_rename_original_title
                                rename_input_ref=rename_input_ref
                                set_ctx_menu=set_ctx_menu
                                set_remove_project_confirm=set_remove_project_confirm
                                toggle_pin=toggle_pin
                                set_delete_confirm=set_delete_confirm
                                select_session=select_session
                                rename_session=rename_session
                                toggle_project_expand=toggle_project_expand
                                new_session_for_project=new_session_for_project
                            />
                        }
                    }
                />
            </div>

            // Overlays
            <SidebarContextMenu
                ctx_menu=ctx_menu
                set_ctx_menu=set_ctx_menu
                pinned_sessions=pinned_sessions
                toggle_pin=toggle_pin
                set_renaming_sid=set_renaming_sid
                set_rename_text=set_rename_text
                set_rename_original_title=set_rename_original_title
                set_delete_confirm=set_delete_confirm
            />

            <DeleteSessionModal
                delete_confirm=delete_confirm
                set_delete_confirm=set_delete_confirm
                delete_loading=delete_loading
                do_delete=do_delete
            />

            <RemoveProjectModal
                remove_project_confirm=remove_project_confirm
                set_remove_project_confirm=set_remove_project_confirm
                remove_project_loading=remove_project_loading
                do_remove_project=do_remove_project
            />

            // Add Project button
            <div class="sb-add-project">
                <button
                    class="sb-add-project-btn"
                    on:click=move |_| modal_state.open(ModalName::AddProject)
                    title="Add Project"
                >
                    <IconFolderPlus size=14 />
                    <span>"Add Project"</span>
                </button>
            </div>
        </aside>
    }
}
