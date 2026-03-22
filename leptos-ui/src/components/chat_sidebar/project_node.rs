//! Per-project node in the sidebar — header + session list.

use super::project_sessions::ProjectSessions;
use super::types::{ContextMenuState, DeleteConfirm, RemoveProjectConfirm};
use crate::components::icons::*;
use crate::hooks::use_sse_state::SseState;
use crate::hooks::use_swipe_reveal::{use_swipe_reveal, SwipeConfig};
use crate::types::api::SessionInfo;
use leptos::prelude::*;
use std::collections::HashSet;

#[component]
pub fn ProjectNode(
    idx: usize,
    project_name: String,
    git_branch: String,
    sessions: Vec<SessionInfo>,
    sse: SseState,
    active_project_idx: Memo<usize>,
    active_session_id: Memo<Option<String>>,
    expanded_project: ReadSignal<Option<usize>>,
    pinned_sessions: ReadSignal<HashSet<String>>,
    expanded_subagents: ReadSignal<Option<String>>,
    set_expanded_subagents: WriteSignal<Option<String>>,
    show_more_project: ReadSignal<Option<usize>>,
    set_show_more_project: WriteSignal<Option<usize>>,
    search_query: ReadSignal<String>,
    renaming_sid: ReadSignal<Option<String>>,
    set_renaming_sid: WriteSignal<Option<String>>,
    rename_text: ReadSignal<String>,
    set_rename_text: WriteSignal<String>,
    rename_original_title: ReadSignal<String>,
    rename_input_ref: NodeRef<leptos::html::Input>,
    set_ctx_menu: WriteSignal<Option<ContextMenuState>>,
    set_remove_project_confirm: WriteSignal<Option<RemoveProjectConfirm>>,
    toggle_pin: Callback<String>,
    set_delete_confirm: WriteSignal<Option<DeleteConfirm>>,
    set_rename_original_title: WriteSignal<String>,
    select_session: Callback<(usize, String)>,
    rename_session: Callback<(String, String)>,
    toggle_project_expand: Callback<usize>,
    new_session_for_project: Callback<usize>,
) -> impl IntoView {
    let sessions_stored = StoredValue::new(sessions);
    let project_name_header = project_name.clone();

    // Swipe state for remove action
    let swipe = use_swipe_reveal(SwipeConfig {
        actions_width: 48.0,
    });
    let on_ts = swipe.on_touch_start();
    let on_tm = swipe.on_touch_move();
    let on_te = swipe.on_touch_end();

    view! {
        <div class="sb-project">
            <div
                class=move || swipe.container_class()
                on:touchstart=move |ev| on_ts(ev)
                on:touchmove=move |ev| on_tm(ev)
                on:touchend=move |ev| on_te(ev)
            >
                <div class="swipe-row-actions">
                    {
                        let name = project_name.clone();
                        view! {
                            <button
                                class="swipe-action-btn swipe-action-danger"
                                title="Remove project" aria-label="Remove project"
                                on:click={
                                    let name = name.clone();
                                    move |ev: web_sys::MouseEvent| {
                                        ev.stop_propagation();
                                        set_remove_project_confirm.set(Some(RemoveProjectConfirm {
                                            project_idx: idx, project_name: name.clone(),
                                        }));
                                        swipe.close();
                                    }
                                }
                            ><IconTrash2 size=14 /></button>
                        }
                    }
                </div>
                <div class="swipe-row-content" style=move || swipe.content_style()>
                    <button
                        class=move || {
                            if idx == active_project_idx.get() { "sb-project-header active".to_string() }
                            else { "sb-project-header".to_string() }
                        }
                        on:click=move |_| toggle_project_expand.run(idx)
                    >
                        <span class="sb-project-chevron">
                            {move || {
                                if expanded_project.get() == Some(idx) {
                                    view! { <IconChevronDown size=14 /> }.into_any()
                                } else {
                                    view! { <IconChevronRight size=14 /> }.into_any()
                                }
                            }}
                        </span>
                        <span class="sb-project-name">{project_name_header}</span>
                        {render_project_indicator(sse, sessions_stored)}
                        {if !git_branch.is_empty() {
                            let branch = git_branch.clone();
                            Some(view! {
                                <span class="sb-project-branch"><IconGitBranch size=10 />{branch}</span>
                            })
                        } else { None }}
                        <span class="sb-project-count">{
                            let sess = sessions_stored;
                            move || sess.get_value().iter().filter(|s| s.parent_id.is_empty()).count()
                        }</span>
                    </button>
                </div>
            </div>

            <ProjectSessions
                idx=idx
                sessions_stored=sessions_stored
                active_session_id=active_session_id
                pinned_sessions=pinned_sessions
                busy_sessions=sse.busy_sessions
                error_sessions=sse.error_sessions
                input_sessions=sse.input_sessions
                unseen_sessions=sse.unseen_sessions
                expanded_project=expanded_project
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
                toggle_pin=toggle_pin
                set_delete_confirm=set_delete_confirm
                select_session=select_session
                rename_session=rename_session
                new_session_for_project=new_session_for_project
            />
        </div>
    }
}

/// Reactive project-level status indicator (busy > input > error > unseen).
fn render_project_indicator(
    sse: SseState,
    sessions_stored: StoredValue<Vec<SessionInfo>>,
) -> impl IntoView {
    let (busy, err, inp, uns) = (
        sse.busy_sessions,
        sse.error_sessions,
        sse.input_sessions,
        sse.unseen_sessions,
    );
    move || {
        let (busy_set, err_set, inp_set, uns_set) = (busy.get(), err.get(), inp.get(), uns.get());
        let sids = sessions_stored.get_value();
        if sids.iter().any(|s| busy_set.contains(&s.id)) {
            return Some(view! { <span class="sb-indicator sb-indicator-busy" /> }.into_any());
        }
        if sids.iter().any(|s| inp_set.contains(&s.id)) {
            return Some(view! { <span class="sb-indicator sb-indicator-input" /> }.into_any());
        }
        if sids.iter().any(|s| err_set.contains(&s.id)) {
            return Some(view! { <span class="sb-indicator sb-indicator-error" /> }.into_any());
        }
        if sids.iter().any(|s| uns_set.contains(&s.id)) {
            return Some(view! { <span class="sb-indicator sb-indicator-unseen" /> }.into_any());
        }
        None
    }
}
