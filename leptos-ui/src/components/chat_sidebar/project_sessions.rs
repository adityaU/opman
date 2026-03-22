//! Expanded session list beneath a project header.

use super::session_row::SessionRow;
use super::types::{ContextMenuState, DeleteConfirm, MAX_VISIBLE_SESSIONS};
use crate::components::icons::*;
use crate::types::api::SessionInfo;
use leptos::prelude::*;
use std::collections::{HashMap, HashSet};

#[component]
pub fn ProjectSessions(
    idx: usize,
    sessions_stored: StoredValue<Vec<SessionInfo>>,
    active_session_id: Memo<Option<String>>,
    pinned_sessions: ReadSignal<HashSet<String>>,
    busy_sessions: ReadSignal<HashSet<String>>,
    error_sessions: ReadSignal<HashSet<String>>,
    input_sessions: ReadSignal<HashSet<String>>,
    unseen_sessions: ReadSignal<HashSet<String>>,
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
    set_rename_original_title: WriteSignal<String>,
    rename_input_ref: NodeRef<leptos::html::Input>,
    set_ctx_menu: WriteSignal<Option<ContextMenuState>>,
    toggle_pin: Callback<String>,
    set_delete_confirm: WriteSignal<Option<DeleteConfirm>>,
    select_session: Callback<(usize, String)>,
    rename_session: Callback<(String, String)>,
    new_session_for_project: Callback<usize>,
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
                <button
                    class="sb-session sb-new-session-row"
                    on:click=move |_| new_session_for_project.run(idx)
                >
                    <div class="sb-session-icon">
                        <IconPlus size=14 />
                    </div>
                    <div class="sb-session-info">
                        <span class="sb-session-title">"New Session"</span>
                    </div>
                </button>
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
                            error_sessions=error_sessions input_sessions=input_sessions
                            unseen_sessions=unseen_sessions
                            expanded_subagents=expanded_subagents
                            set_expanded_subagents=set_expanded_subagents
                            renaming_sid=renaming_sid set_renaming_sid=set_renaming_sid
                            rename_text=rename_text set_rename_text=set_rename_text
                            rename_original_title=rename_original_title
                            set_rename_original_title=set_rename_original_title
                            rename_input_ref=rename_input_ref
                            set_ctx_menu=set_ctx_menu
                            toggle_pin=toggle_pin
                            set_delete_confirm=set_delete_confirm
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
