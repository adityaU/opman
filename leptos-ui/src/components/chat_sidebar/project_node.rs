//! Per-project node in the sidebar — header + session list.

use super::session_row::SessionRow;
use super::types::{ContextMenuState, RemoveProjectConfirm, MAX_VISIBLE_SESSIONS};
use crate::components::icons::*;
use crate::hooks::use_sse_state::SseState;
use crate::types::api::SessionInfo;
use leptos::prelude::*;
use std::collections::{HashMap, HashSet};

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
    select_session: Callback<(usize, String)>,
    rename_session: Callback<(String, String)>,
    toggle_project_expand: Callback<usize>,
    new_session_for_project: Callback<usize>,
) -> impl IntoView {
    let sessions_stored = StoredValue::new(sessions);
    let project_name_header = project_name.clone();

    view! {
        <div class="sb-project">
            <div class="sb-project-header-row">
                <button
                    class=move || {
                        if idx == active_project_idx.get() { "sb-project-header active".to_string() }
                        else { "sb-project-header".to_string() }
                    }
                    style="flex:1;min-width:0"
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
                    {
                        let busy = sse.busy_sessions;
                        let sess = sessions_stored;
                        move || {
                            let busy_set = busy.get();
                            if sess.get_value().iter().any(|s| busy_set.contains(&s.id)) {
                                Some(view! { <span class="sb-activity-dot" /> })
                            } else { None }
                        }
                    }
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
                <button
                    class="sb-project-new sb-icon-btn"
                    title="New session in this project"
                    aria-label="New session"
                    on:click=move |ev: web_sys::MouseEvent| {
                        ev.stop_propagation();
                        new_session_for_project.run(idx);
                    }
                >
                    <IconPlus size=12 />
                </button>
                {
                    let name = project_name.clone();
                    view! {
                        <button
                            class="sb-project-remove sb-icon-btn"
                            title="Remove project" aria-label="Remove project"
                            on:click={
                                let name = name.clone();
                                move |ev: web_sys::MouseEvent| {
                                    ev.stop_propagation();
                                    set_remove_project_confirm.set(Some(RemoveProjectConfirm {
                                        project_idx: idx, project_name: name.clone(),
                                    }));
                                }
                            }
                        ><IconX size=12 /></button>
                    }
                }
            </div>

            <ProjectSessions
                idx=idx
                sessions_stored=sessions_stored
                active_session_id=active_session_id
                pinned_sessions=pinned_sessions
                busy_sessions=sse.busy_sessions
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
                rename_input_ref=rename_input_ref
                set_ctx_menu=set_ctx_menu
                select_session=select_session
                rename_session=rename_session
            />
        </div>
    }
}

#[component]
fn ProjectSessions(
    idx: usize,
    sessions_stored: StoredValue<Vec<SessionInfo>>,
    active_session_id: Memo<Option<String>>,
    pinned_sessions: ReadSignal<HashSet<String>>,
    busy_sessions: ReadSignal<HashSet<String>>,
    expanded_project: ReadSignal<Option<usize>>,
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
    select_session: Callback<(usize, String)>,
    rename_session: Callback<(String, String)>,
) -> impl IntoView {
    move || {
        if expanded_project.get() != Some(idx) {
            return None;
        }

        let pinned = pinned_sessions.get();
        let query = search_query.get().to_lowercase();
        let mut parents: Vec<SessionInfo> = Vec::new();
        let mut children_map: HashMap<String, Vec<SessionInfo>> = HashMap::new();

        for s in sessions_stored.get_value().iter() {
            if s.parent_id.is_empty() {
                parents.push(s.clone());
            } else {
                children_map
                    .entry(s.parent_id.clone())
                    .or_default()
                    .push(s.clone());
            }
        }

        parents.sort_by(|a, b| {
            let ap = if pinned.contains(&a.id) { 1 } else { 0 };
            let bp = if pinned.contains(&b.id) { 1 } else { 0 };
            if ap != bp {
                return bp.cmp(&ap);
            }
            b.time
                .updated
                .partial_cmp(&a.time.updated)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let filtered: Vec<SessionInfo> = if query.is_empty() {
            parents
        } else {
            parents
                .into_iter()
                .filter(|s| {
                    let t = if s.title.is_empty() { &s.id } else { &s.title };
                    t.to_lowercase().contains(&query)
                })
                .collect()
        };

        let show_more = show_more_project.get() == Some(idx);
        let visible: Vec<SessionInfo> = if show_more {
            filtered.clone()
        } else {
            filtered
                .iter()
                .take(MAX_VISIBLE_SESSIONS)
                .cloned()
                .collect()
        };
        let has_more = filtered.len() > MAX_VISIBLE_SESSIONS && !show_more;
        let remaining = filtered.len().saturating_sub(MAX_VISIBLE_SESSIONS);

        Some(view! {
            <div class="sb-sessions">
                {if visible.is_empty() {
                    Some(view! {
                        <div class="sb-empty">
                            {if !query.is_empty() { "No matching sessions" } else { "No sessions yet" }}
                        </div>
                    }.into_any())
                } else { None }}
                {visible.into_iter().map(|session| {
                    let sid = session.id.clone();
                    let subagents = children_map.get(&sid).cloned().unwrap_or_default();
                    view! {
                        <SessionRow
                            session=session subagents=subagents project_idx=idx
                            active_session_id=active_session_id
                            pinned_sessions=pinned_sessions busy_sessions=busy_sessions
                            expanded_subagents=expanded_subagents
                            set_expanded_subagents=set_expanded_subagents
                            renaming_sid=renaming_sid set_renaming_sid=set_renaming_sid
                            rename_text=rename_text set_rename_text=set_rename_text
                            rename_original_title=rename_original_title
                            rename_input_ref=rename_input_ref
                            set_ctx_menu=set_ctx_menu
                            select_session=select_session rename_session=rename_session
                        />
                    }
                }).collect::<Vec<_>>()}
                {if has_more {
                    Some(view! {
                        <button class="sb-show-more" on:click=move |_| set_show_more_project.set(Some(idx))>
                            {format!("Show {} more", remaining)}
                        </button>
                    })
                } else { None }}
            </div>
        })
    }
}
