//! "Open Sessions" section — pinned at the top of the sidebar.
//! Sessions placed here persist until explicitly removed.
//! Each entry shows the session title, project name tag, and status indicator.

use std::collections::HashSet;

use leptos::prelude::*;

use crate::components::icons::*;
use crate::types::api::ProjectInfo;

use super::types::{format_time, indicator_class, ContextMenuState};

/// Lightweight struct carrying everything needed to render one open-session row.
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
    select_session: Callback<(usize, String)>,
    toggle_open_session: Callback<String>,
    set_ctx_menu: WriteSignal<Option<ContextMenuState>>,
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
                if !open.contains(&s.id) {
                    continue;
                }
                if !s.parent_id.is_empty() {
                    continue; // skip subagents
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

        // Sort by updated time (most recent first via string comparison isn't
        // reliable — but format_time produces relative strings).  We re-sort by
        // looking up the raw `time.updated` from projects instead.
        entries.sort_by(|a, b| {
            let ta = raw_updated(&projs, &a.sid);
            let tb = raw_updated(&projs, &b.sid);
            tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
        });

        if entries.is_empty() {
            return None; // all open IDs stale (sessions deleted)
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
                        select_session,
                        toggle_open_session,
                        set_ctx_menu,
                    )
                }).collect::<Vec<_>>()}
            </div>
        })
    }
}

fn open_session_row(
    e: OpenEntry,
    active_session_id: Memo<Option<String>>,
    busy_sessions: ReadSignal<HashSet<String>>,
    error_sessions: ReadSignal<HashSet<String>>,
    input_sessions: ReadSignal<HashSet<String>>,
    unseen_sessions: ReadSignal<HashSet<String>>,
    select_session: Callback<(usize, String)>,
    toggle_open_session: Callback<String>,
    set_ctx_menu: WriteSignal<Option<ContextMenuState>>,
) -> impl IntoView {
    let (sid_cls, sid_click, sid_ind, sid_remove, sid_ctx) = (
        e.sid.clone(),
        e.sid.clone(),
        e.sid.clone(),
        e.sid.clone(),
        e.sid.clone(),
    );
    let title_ctx = e.title.clone();
    let pidx = e.project_idx;

    view! {
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
                <IconMessageCircle size=14 />
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
                        toggle_open_session.run(sid.clone());
                    }
                }
            >
                <IconX size=10 />
            </span>
        </button>
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
