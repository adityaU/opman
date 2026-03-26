//! "Open Sessions" section — pinned at the top of the sidebar.
//! Sessions are auto-added when opened from the project tree.
//! Each row has: title, project tag, status indicator, remove button,
//! context menu (desktop), swipe-to-reveal pin/rename/delete (mobile).

use std::collections::HashSet;

use leptos::prelude::*;

use crate::components::icons::*;
use crate::hooks::use_swipe_reveal::{use_swipe_reveal, SwipeConfig};
use crate::types::api::ProjectInfo;

use super::types::{format_time, indicator_class, ContextMenuState, DeleteConfirm};

/// Width of 3 action buttons (matches session_row.rs).
const SWIPE_ACTIONS_WIDTH: f64 = 128.0;

/// Everything needed to render one open-session row.
#[derive(Clone)]
struct OpenEntry {
    sid: String,
    title: String,
    project_name: String,
    project_idx: usize,
    updated: String,
}

#[component]
pub fn OpenSessionsSection(
    projects: Memo<Vec<ProjectInfo>>,
    open_sessions: ReadSignal<HashSet<String>>,
    active_session_id: Memo<Option<String>>,
    busy_sessions: ReadSignal<HashSet<String>>,
    error_sessions: ReadSignal<HashSet<String>>,
    input_sessions: ReadSignal<HashSet<String>>,
    unseen_sessions: ReadSignal<HashSet<String>>,
    pinned_sessions: ReadSignal<HashSet<String>>,
    select_session: Callback<(usize, String)>,
    remove_open_session: Callback<String>,
    toggle_pin: Callback<String>,
    set_ctx_menu: WriteSignal<Option<ContextMenuState>>,
    set_renaming_sid: WriteSignal<Option<String>>,
    set_rename_text: WriteSignal<String>,
    set_rename_original_title: WriteSignal<String>,
    set_delete_confirm: WriteSignal<Option<DeleteConfirm>>,
) -> impl IntoView {
    move || {
        let open = open_sessions.get();
        if open.is_empty() {
            return None;
        }

        let projs = projects.get();
        let mut entries: Vec<OpenEntry> = Vec::with_capacity(open.len());

        for p in &projs {
            for s in &p.sessions {
                if !open.contains(&s.id) || !s.parent_id.is_empty() {
                    continue;
                }
                entries.push(OpenEntry {
                    sid: s.id.clone(),
                    title: if s.title.is_empty() {
                        s.id[..s.id.len().min(12)].to_string()
                    } else {
                        s.title.clone()
                    },
                    project_name: p.name.clone(),
                    project_idx: p.index,
                    updated: format_time(s.time.updated),
                });
            }
        }

        entries.sort_by(|a, b| {
            let ta = raw_updated(&projs, &a.sid);
            let tb = raw_updated(&projs, &b.sid);
            tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
        });

        if entries.is_empty() {
            return None;
        }

        Some(view! {
            <div class="sb-open-sessions">
                <div class="sb-open-header">
                    <IconLayers size=12 />
                    <span>"Open Sessions"</span>
                </div>
                {entries.into_iter().map(|e| {
                    open_session_row(
                        e,
                        active_session_id,
                        busy_sessions,
                        error_sessions,
                        input_sessions,
                        unseen_sessions,
                        pinned_sessions,
                        select_session,
                        remove_open_session,
                        toggle_pin,
                        set_ctx_menu,
                        set_renaming_sid,
                        set_rename_text,
                        set_rename_original_title,
                        set_delete_confirm,
                    )
                }).collect::<Vec<_>>()}
            </div>
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn open_session_row(
    e: OpenEntry,
    active_session_id: Memo<Option<String>>,
    busy_sessions: ReadSignal<HashSet<String>>,
    error_sessions: ReadSignal<HashSet<String>>,
    input_sessions: ReadSignal<HashSet<String>>,
    unseen_sessions: ReadSignal<HashSet<String>>,
    pinned_sessions: ReadSignal<HashSet<String>>,
    select_session: Callback<(usize, String)>,
    remove_open_session: Callback<String>,
    toggle_pin: Callback<String>,
    set_ctx_menu: WriteSignal<Option<ContextMenuState>>,
    set_renaming_sid: WriteSignal<Option<String>>,
    set_rename_text: WriteSignal<String>,
    set_rename_original_title: WriteSignal<String>,
    set_delete_confirm: WriteSignal<Option<DeleteConfirm>>,
) -> impl IntoView {
    let sid = e.sid.clone();
    let pidx = e.project_idx;

    // Clones for various closures
    let (sid_cls, sid_click, sid_ind, sid_remove, sid_ctx) = (
        sid.clone(),
        sid.clone(),
        sid.clone(),
        sid.clone(),
        sid.clone(),
    );
    let (sid_swipe_pin, sid_swipe_rename, sid_swipe_delete) =
        (sid.clone(), sid.clone(), sid.clone());
    let title_ctx = e.title.clone();
    let (title_swipe_rename, title_swipe_delete) = (e.title.clone(), e.title.clone());

    // Swipe state for mobile
    let swipe = use_swipe_reveal(SwipeConfig {
        actions_width: SWIPE_ACTIONS_WIDTH,
    });
    let on_ts = swipe.on_touch_start();
    let on_tm = swipe.on_touch_move();
    let on_te = swipe.on_touch_end();

    view! {
        <div
            class=move || swipe.container_class()
            on:touchstart=move |ev| on_ts(ev)
            on:touchmove=move |ev| on_tm(ev)
            on:touchend=move |ev| on_te(ev)
        >
            // Swipe action tray (behind content)
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

            // Front content layer
            <div class="swipe-row-content" style=move || swipe.content_style()>
                <button
                    class=move || {
                        let mut c = String::from("sb-session sb-open-row");
                        if active_session_id.get().as_deref() == Some(sid_cls.as_str()) {
                            c.push_str(" active");
                        }
                        c
                    }
                    on:click={
                        let sid = sid_click;
                        move |_| select_session.run((pidx, sid.clone()))
                    }
                    on:contextmenu={
                        let sid = sid_ctx;
                        let title = title_ctx;
                        move |ev: web_sys::MouseEvent| {
                            ev.prevent_default();
                            ev.stop_propagation();
                            set_ctx_menu.set(Some(ContextMenuState {
                                session_id: sid.clone(),
                                session_title: title.clone(),
                                x: ev.client_x(),
                                y: ev.client_y(),
                                project_idx: pidx,
                            }));
                        }
                    }
                >
                    <div class="sb-session-icon">
                        {move || {
                            if pinned_sessions.get().contains(&sid) {
                                view! { <IconPin size=12 class="sb-pin-icon" /> }.into_any()
                            } else {
                                view! { <IconMessageCircle size=14 /> }.into_any()
                            }
                        }}
                    </div>
                    <div class="sb-session-info">
                        <span class="sb-session-title">{e.title}</span>
                        <span class="sb-session-meta">
                            <span class="sb-open-project-tag">{e.project_name}</span>
                            {e.updated}
                        </span>
                    </div>
                    {move || {
                        let busy = busy_sessions.get();
                        let inp = input_sessions.get();
                        let err = error_sessions.get();
                        let uns = unseen_sessions.get();
                        let cls = indicator_class(&sid_ind, &busy, &inp, &err, &uns);
                        view! { <span class=cls /> }
                    }}
                    <span
                        class="sb-open-remove"
                        title="Remove from Open Sessions"
                        on:click={
                            let sid = sid_remove;
                            move |ev: web_sys::MouseEvent| {
                                ev.stop_propagation();
                                remove_open_session.run(sid.clone());
                            }
                        }
                    >
                        <IconX size=10 />
                    </span>
                </button>
            </div>
        </div>
    }
}

/// Look up the raw `time.updated` for a session across all projects.
fn raw_updated(projects: &[ProjectInfo], sid: &str) -> f64 {
    for p in projects {
        for s in &p.sessions {
            if s.id == sid {
                return s.time.updated;
            }
        }
    }
    0.0
}
