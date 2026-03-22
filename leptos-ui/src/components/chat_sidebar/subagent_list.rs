use super::types::{format_time, indicator_class};
use crate::components::icons::*;
use crate::types::api::SessionInfo;
use leptos::prelude::*;
use std::collections::HashSet;

/// Expandable subagent list under a parent session row.
#[component]
pub fn SubagentList(
    parent_sid: String,
    subagents: Vec<SessionInfo>,
    project_idx: usize,
    active_session_id: Memo<Option<String>>,
    busy_sessions: ReadSignal<HashSet<String>>,
    error_sessions: ReadSignal<HashSet<String>>,
    input_sessions: ReadSignal<HashSet<String>>,
    unseen_sessions: ReadSignal<HashSet<String>>,
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
        let (busy, err, inp, uns, sess_id) = (
            busy_sessions.get(),
            error_sessions.get(),
            input_sessions.get(),
            unseen_sessions.get(),
            active_session_id.get(),
        );
        Some(view! {
            <div class="sb-subagents">
                {subagents.iter().map(|sub| {
                    let id = sub.id.clone();
                    let (is_busy, is_active) = (busy.contains(&id), sess_id.as_deref() == Some(id.as_str()));
                    let title = if sub.title.is_empty() { sub.id[..sub.id.len().min(12)].to_string() } else { sub.title.clone() };
                    let (time, id_click) = (format_time(sub.time.updated), id.clone());
                    let ind_cls = indicator_class(&id, &busy, &inp, &err, &uns);
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
                            <span class=ind_cls />
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
        })
    })
}
