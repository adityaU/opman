//! Individual session row + subagent list components for the sidebar.

use super::types::{format_time, ContextMenuState};
use crate::components::icons::*;
use crate::types::api::SessionInfo;
use leptos::prelude::*;
use std::collections::HashSet;

#[component]
pub fn SessionRow(
    session: SessionInfo,
    subagents: Vec<SessionInfo>,
    project_idx: usize,
    active_session_id: Memo<Option<String>>,
    pinned_sessions: ReadSignal<HashSet<String>>,
    busy_sessions: ReadSignal<HashSet<String>>,
    expanded_subagents: ReadSignal<Option<String>>,
    set_expanded_subagents: WriteSignal<Option<String>>,
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
    let sid = session.id.clone();
    let has_subagents = !subagents.is_empty();
    let subagent_count = subagents.len();
    let title = if session.title.is_empty() {
        session.id[..session.id.len().min(12)].to_string()
    } else {
        session.title.clone()
    };
    let updated_time = format_time(session.time.updated);
    // Clone IDs/titles for each closure and prop that needs its own copy.
    let sid_class = sid.clone();
    let sid_click = sid.clone();
    let sid_pinned = sid.clone();
    let sid_ctx = sid.clone();
    let title_ctx = title.clone();
    let sid_rename_check = sid.clone();
    let sid_rename_submit = sid.clone();
    let sid_subagent_toggle = sid.clone();
    let sid_subagent_list = sid.clone();
    let title_rename = title.clone();
    let sid_for_actions = sid.clone();
    let title_for_actions = title.clone();
    let sid_busy = sid.clone();
    let sid_actions_check = sid.clone();
    let subagents_class = subagents.clone();
    let subagents_list = subagents.clone();

    view! {
        <div class="sb-session-group">
            <button
                class=move || {
                    let mut c = String::from("sb-session");
                    if active_session_id.get().as_deref() == Some(sid_class.as_str()) { c.push_str(" active"); }
                    let busy = busy_sessions.get();
                    if busy.contains(&sid_class) || subagents_class.iter().any(|s| busy.contains(&s.id)) { c.push_str(" busy"); }
                    c
                }
                on:click={
                    let sid = sid_click;
                    move |_| {
                        if renaming_sid.get_untracked().as_deref() != Some(sid.as_str()) {
                            select_session.run((project_idx, sid.clone()));
                        }
                    }
                }
                on:contextmenu={
                    let sid = sid_ctx;
                    let title = title_ctx;
                    move |ev: web_sys::MouseEvent| {
                        ev.prevent_default();
                        ev.stop_propagation();
                        set_ctx_menu.set(Some(ContextMenuState {
                            session_id: sid.clone(), session_title: title.clone(),
                            x: ev.client_x(), y: ev.client_y(), project_idx,
                        }));
                    }
                }
            >
                <div class="sb-session-icon">
                    {move || {
                        if pinned_sessions.get().contains(&sid_pinned) {
                            view! { <IconPin size=12 class="sb-pin-icon" /> }.into_any()
                        } else {
                            view! { <IconMessageCircle size=14 /> }.into_any()
                        }
                    }}
                </div>

                <div class="sb-session-info">
                    <SessionInfoContent
                        sid=sid_rename_check
                        sid_submit=sid_rename_submit
                        title=title_rename
                        updated_time=updated_time
                        has_subagents=has_subagents
                        subagent_count=subagent_count
                        sid_subagent_toggle=sid_subagent_toggle
                        renaming_sid=renaming_sid
                        set_renaming_sid=set_renaming_sid
                        rename_text=rename_text
                        set_rename_text=set_rename_text
                        rename_original_title=rename_original_title
                        rename_input_ref=rename_input_ref
                        expanded_subagents=expanded_subagents
                        set_expanded_subagents=set_expanded_subagents
                        rename_session=rename_session
                    />
                </div>

                {move || {
                    if busy_sessions.get().contains(&sid_busy) {
                        Some(view! { <span class="sb-busy-indicator" /> })
                    } else { None }
                }}

                // More actions (hidden during rename)
                {
                    let sid = sid_for_actions;
                    let title = title_for_actions;
                    let sid_check = sid_actions_check;
                    move || {
                        if renaming_sid.get().as_deref() == Some(sid_check.as_str()) { return None; }
                        let sid = sid.clone();
                        let title = title.clone();
                        Some(view! {
                            <span class="sb-session-actions" on:click={
                                let sid = sid.clone(); let title = title.clone();
                                move |ev: web_sys::MouseEvent| {
                                    ev.stop_propagation(); ev.prevent_default();
                                    set_ctx_menu.set(Some(ContextMenuState {
                                        session_id: sid.clone(), session_title: title.clone(),
                                        x: ev.client_x(), y: ev.client_y(), project_idx,
                                    }));
                                }
                            }><IconMoreHorizontal size=14 /></span>
                        })
                    }
                }
            </button>

            <SubagentList
                parent_sid=sid_subagent_list
                subagents=subagents_list
                project_idx=project_idx
                active_session_id=active_session_id
                busy_sessions=busy_sessions
                expanded_subagents=expanded_subagents
                select_session=select_session
            />
        </div>
    }
}

/// Session title/meta/rename-input content (extracted to stay under 300 lines).
#[component]
fn SessionInfoContent(
    sid: String,
    sid_submit: String,
    title: String,
    updated_time: String,
    has_subagents: bool,
    subagent_count: usize,
    sid_subagent_toggle: String,
    renaming_sid: ReadSignal<Option<String>>,
    set_renaming_sid: WriteSignal<Option<String>>,
    rename_text: ReadSignal<String>,
    set_rename_text: WriteSignal<String>,
    rename_original_title: ReadSignal<String>,
    rename_input_ref: NodeRef<leptos::html::Input>,
    expanded_subagents: ReadSignal<Option<String>>,
    set_expanded_subagents: WriteSignal<Option<String>>,
    rename_session: Callback<(String, String)>,
) -> impl IntoView {
    move || {
        if renaming_sid.get().as_deref() == Some(sid.as_str()) {
            let sid_s = sid_submit.clone();
            view! {
                <input
                    node_ref=rename_input_ref
                    class="sb-rename-input" type="text"
                    prop:value=rename_text
                    on:input=move |ev| set_rename_text.set(event_target_value(&ev))
                    on:keydown={
                        let sid = sid_s.clone();
                        move |ev: web_sys::KeyboardEvent| {
                            if ev.key() == "Enter" {
                                ev.prevent_default();
                                let t = rename_text.get_untracked();
                                if !t.trim().is_empty() { rename_session.run((sid.clone(), t.trim().to_string())); }
                                set_renaming_sid.set(None);
                            } else if ev.key() == "Escape" { set_renaming_sid.set(None); }
                        }
                    }
                    on:blur={
                        let sid_blur = sid_s.clone();
                        move |_| {
                            let sid = sid_blur.clone();
                            leptos::task::spawn_local(async move {
                                gloo_timers::future::sleep(std::time::Duration::from_millis(150)).await;
                                let t = rename_text.get_untracked().trim().to_string();
                                if !t.is_empty() && t != rename_original_title.get_untracked() {
                                    rename_session.run((sid, t));
                                } else { set_renaming_sid.set(None); }
                            });
                        }
                    }
                    on:click=move |ev: web_sys::MouseEvent| { ev.stop_propagation(); }
                />
            }.into_any()
        } else {
            let t = title.clone();
            let u = updated_time.clone();
            let sid_t = sid_subagent_toggle.clone();
            view! {
                <>
                    <span class="sb-session-title">{t}</span>
                    <span class="sb-session-meta">
                        {u}
                        {if has_subagents {
                            let sid_t2 = sid_t.clone();
                            Some(view! {
                                <span class="sb-subagent-badge" on:click={
                                    move |ev: web_sys::MouseEvent| {
                                        ev.stop_propagation();
                                        let cur = expanded_subagents.get_untracked();
                                        if cur.as_deref() == Some(sid_t2.as_str()) {
                                            set_expanded_subagents.set(None);
                                        } else { set_expanded_subagents.set(Some(sid_t2.clone())); }
                                    }
                                } title=format!("{} subagent{}", subagent_count, if subagent_count > 1 { "s" } else { "" })>
                                    <IconZap size=8 />{subagent_count}
                                </span>
                            })
                        } else { None }}
                    </span>
                </>
            }.into_any()
        }
    }
}

/// Renders the expanded subagent list below a parent session.
#[component]
fn SubagentList(
    parent_sid: String,
    subagents: Vec<SessionInfo>,
    project_idx: usize,
    active_session_id: Memo<Option<String>>,
    busy_sessions: ReadSignal<HashSet<String>>,
    expanded_subagents: ReadSignal<Option<String>>,
    select_session: Callback<(usize, String)>,
) -> impl IntoView {
    if subagents.is_empty() {
        return None;
    }
    Some(move || {
        if expanded_subagents.get().as_deref() != Some(parent_sid.as_str()) {
            return None;
        }
        let busy = busy_sessions.get();
        let sess_id = active_session_id.get();
        Some(view! {
            <div class="sb-subagents">
                {subagents.iter().map(|sub| {
                    let id = sub.id.clone();
                    let is_busy = busy.contains(&id);
                    let is_active = sess_id.as_deref() == Some(id.as_str());
                    let title = if sub.title.is_empty() { sub.id[..sub.id.len().min(12)].to_string() } else { sub.title.clone() };
                    let time = format_time(sub.time.updated);
                    let id_click = id.clone();
                    view! {
                        <button
                            class=move || {
                                let mut c = String::from("sb-session sb-session-sub");
                                if is_active { c.push_str(" active"); }
                                if is_busy { c.push_str(" busy"); }
                                c
                            }
                            on:click={ let id = id_click.clone(); move |_| select_session.run((project_idx, id.clone())) }
                        >
                            <div class="sb-session-icon sub"><IconZap size=12 /></div>
                            <div class="sb-session-info">
                                <span class="sb-session-title">{title}</span>
                                <span class="sb-session-meta">{time}</span>
                            </div>
                            {if is_busy { Some(view! { <span class="sb-busy-indicator" /> }) } else { None }}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
        })
    })
}
