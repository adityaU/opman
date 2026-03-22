use super::session_button::SessionButton;
use super::subagent_list::SubagentList;
use super::types::{format_time, ContextMenuState, DeleteConfirm};
use crate::components::icons::*;
use crate::hooks::use_swipe_reveal::{use_swipe_reveal, SwipeConfig};
use crate::types::api::SessionInfo;
use leptos::prelude::*;
use std::collections::HashSet;

/// Width of 3 action buttons (34px each) + gaps + tray padding.
const SWIPE_ACTIONS_WIDTH: f64 = 128.0;

#[component]
pub fn SessionRow(
    session: SessionInfo,
    subagents: Vec<SessionInfo>,
    project_idx: usize,
    active_session_id: Memo<Option<String>>,
    pinned_sessions: ReadSignal<HashSet<String>>,
    busy_sessions: ReadSignal<HashSet<String>>,
    error_sessions: ReadSignal<HashSet<String>>,
    input_sessions: ReadSignal<HashSet<String>>,
    unseen_sessions: ReadSignal<HashSet<String>>,
    expanded_subagents: ReadSignal<Option<String>>,
    set_expanded_subagents: WriteSignal<Option<String>>,
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

    // Clones for closures
    let (sid_class, sid_click, sid_pinned, sid_ctx) =
        (sid.clone(), sid.clone(), sid.clone(), sid.clone());
    let (sid_rename_check, sid_rename_submit, sid_subagent_toggle, sid_subagent_list) =
        (sid.clone(), sid.clone(), sid.clone(), sid.clone());
    let (sid_for_actions, sid_busy, sid_actions_check) = (sid.clone(), sid.clone(), sid.clone());
    let (title_ctx, title_rename, title_for_actions) =
        (title.clone(), title.clone(), title.clone());
    let (subagents_class, subagents_ind, subagents_list) =
        (subagents.clone(), subagents.clone(), subagents.clone());

    // Swipe state (Copy, no clones needed)
    let swipe = use_swipe_reveal(SwipeConfig {
        actions_width: SWIPE_ACTIONS_WIDTH,
    });
    let (sid_swipe_pin, sid_swipe_rename, sid_swipe_delete) =
        (sid.clone(), sid.clone(), sid.clone());
    let (title_swipe_rename, title_swipe_delete) = (title.clone(), title.clone());

    let on_ts = swipe.on_touch_start();
    let on_tm = swipe.on_touch_move();
    let on_te = swipe.on_touch_end();

    view! {
        <div class="sb-session-group">
            <div
                class=move || swipe.container_class()
                on:touchstart=move |ev| on_ts(ev)
                on:touchmove=move |ev| on_tm(ev)
                on:touchend=move |ev| on_te(ev)
            >
                // Action tray (behind content)
                <div class="swipe-row-actions">
                    <button
                        class="swipe-action-btn swipe-action-primary"
                        title="Pin / Unpin"
                        on:click=move |ev: web_sys::MouseEvent| {
                            ev.stop_propagation();
                            toggle_pin.run(sid_swipe_pin.clone());
                            swipe.close();
                        }
                    ><IconPin size=14 /></button>
                    <button
                        class="swipe-action-btn"
                        title="Rename"
                        on:click=move |ev: web_sys::MouseEvent| {
                            ev.stop_propagation();
                            set_rename_text.set(title_swipe_rename.clone());
                            set_rename_original_title.set(title_swipe_rename.clone());
                            set_renaming_sid.set(Some(sid_swipe_rename.clone()));
                            swipe.close();
                        }
                    ><IconPencil size=14 /></button>
                    <button
                        class="swipe-action-btn swipe-action-danger"
                        title="Delete"
                        on:click=move |ev: web_sys::MouseEvent| {
                            ev.stop_propagation();
                            set_delete_confirm.set(Some(DeleteConfirm {
                                session_id: sid_swipe_delete.clone(),
                                session_title: title_swipe_delete.clone(),
                            }));
                            swipe.close();
                        }
                    ><IconTrash2 size=14 /></button>
                </div>

                // Main session button (front content layer)
                <div class="swipe-row-content" style=move || swipe.content_style()>
                    <SessionButton
                        sid_class=sid_class
                        sid_click=sid_click
                        sid_pinned=sid_pinned
                        sid_ctx=sid_ctx
                        sid_rename_check=sid_rename_check
                        sid_rename_submit=sid_rename_submit
                        sid_subagent_toggle=sid_subagent_toggle
                        sid_for_actions=sid_for_actions
                        sid_busy=sid_busy
                        sid_actions_check=sid_actions_check
                        title_ctx=title_ctx
                        title_rename=title_rename
                        title_for_actions=title_for_actions
                        updated_time=updated_time
                        has_subagents=has_subagents
                        subagent_count=subagent_count
                        subagents_class=subagents_class
                        subagents_ind=subagents_ind
                        project_idx=project_idx
                        active_session_id=active_session_id
                        pinned_sessions=pinned_sessions
                        busy_sessions=busy_sessions
                        error_sessions=error_sessions
                        input_sessions=input_sessions
                        unseen_sessions=unseen_sessions
                        expanded_subagents=expanded_subagents
                        set_expanded_subagents=set_expanded_subagents
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
            </div>

            <SubagentList
                parent_sid=sid_subagent_list
                subagents=subagents_list
                project_idx=project_idx
                active_session_id=active_session_id
                busy_sessions=busy_sessions
                error_sessions=error_sessions
                input_sessions=input_sessions
                unseen_sessions=unseen_sessions
                expanded_subagents=expanded_subagents
                select_session=select_session
            />
        </div>
    }
}
