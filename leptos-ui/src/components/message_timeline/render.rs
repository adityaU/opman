//! Render helpers for the MessageTimeline: keyed and virtual item rendering.

use leptos::prelude::*;
use std::collections::HashSet;

use crate::components::message_turn::{MessageGroup, MessageTurn};
use crate::types::api::SessionInfo;

use super::virtual_list::VirtualState;

type SubMsgsMap = crate::components::tool_call::SubagentMessagesMap;

/// Build the view for a single MessageTurn.
pub fn render_turn_view(
    group: MessageGroup,
    match_ids: Option<HashSet<String>>,
    active_match: Option<String>,
    pending_id: String,
    bm_cb: Option<Callback<String, bool>>,
    toggle_bm_cb: Option<Callback<(String, String, String, String)>>,
    sid: Option<String>,
    cs: Vec<SessionInfo>,
    sub_msgs: ReadSignal<SubMsgsMap>,
) -> impl IntoView {
    view! {
        <MessageTurn
            group=group
            child_sessions=cs
            subagent_messages=sub_msgs
            search_match_ids=match_ids
            active_search_match_id=active_match
            pending_assistant_id=pending_id
            is_bookmarked=bm_cb
            on_toggle_bookmark=toggle_bm_cb
            session_id=sid
        />
    }
}

/// Non-virtual keyed rendering: `<For>` for stable groups + reactive last group.
pub fn render_keyed_items(
    stable_groups: Memo<Vec<MessageGroup>>,
    last_group: Memo<Option<MessageGroup>>,
    search_match_ids: Option<ReadSignal<HashSet<String>>>,
    active_search_match_id: Option<ReadSignal<Option<String>>>,
    pending_assistant_id: Memo<Option<String>>,
    child_sessions: Memo<Vec<SessionInfo>>,
    subagent_messages: ReadSignal<SubMsgsMap>,
    bm_cb: Option<Callback<String, bool>>,
    toggle_bm_cb: Option<Callback<(String, String, String, String)>>,
    sid_val: Option<String>,
) -> AnyView {
    let match_ids_signal = search_match_ids;
    let active_match_signal = active_search_match_id;

    let bm_for = bm_cb;
    let toggle_for = toggle_bm_cb.clone();
    let sid_for = sid_val.clone();
    let bm_last = bm_cb;
    let toggle_last = toggle_bm_cb;
    let sid_last = sid_val;

    view! {
        // Stable groups — keyed, only new groups create DOM.
        // All reactive reads here use get_untracked() because stable rows
        // should NOT re-execute when signals change. The subagent_messages
        // signal is passed through to SubagentSession which reads it reactively.
        <For
            each=move || stable_groups.get()
            key=|group| group.key.clone()
            children={
                let bm = bm_for;
                let toggle = toggle_for.clone();
                let sid = sid_for.clone();
                move |group: MessageGroup| {
                    let match_ids = match_ids_signal.map(|s| s.get_untracked()).unwrap_or_default();
                    let active_match = active_match_signal.and_then(|s| s.get_untracked());
                    let pending_id = pending_assistant_id.get_untracked().unwrap_or_default();
                    let cs = child_sessions.get_untracked();
                    let key = group.key.clone();
                    view! {
                        <div class="message-turn-wrap" data-group-key=key>
                            {render_turn_view(
                                group, Some(match_ids), active_match,
                                pending_id, bm, toggle.clone(), sid.clone(),
                                cs, subagent_messages,
                            )}
                        </div>
                    }
                }
            }
        />

        // Last group — reactive, updates on streaming
        {move || {
            let Some(group) = last_group.get() else { return None };
            let match_ids = match_ids_signal.map(|s| s.get()).unwrap_or_default();
            let active_match = active_match_signal.and_then(|s| s.get());
            let pending_id = pending_assistant_id.get().unwrap_or_default();
            let cs = child_sessions.get();
            let key = group.key.clone();
            Some(view! {
                <div class="message-turn-wrap" data-group-key=key>
                    {render_turn_view(
                        group, Some(match_ids), active_match,
                        pending_id, bm_last, toggle_last.clone(), sid_last.clone(),
                        cs, subagent_messages,
                    )}
                </div>
            })
        }}
    }
    .into_any()
}

/// Virtual rendering: only render groups in [virtual_start, virtual_end).
pub fn render_virtual_items(
    vs: VirtualState,
    groups: Memo<Vec<MessageGroup>>,
    search_match_ids: Option<ReadSignal<HashSet<String>>>,
    active_search_match_id: Option<ReadSignal<Option<String>>>,
    pending_assistant_id: Memo<Option<String>>,
    child_sessions: Memo<Vec<SessionInfo>>,
    subagent_messages: ReadSignal<SubMsgsMap>,
    bm_cb: Option<Callback<String, bool>>,
    toggle_bm_cb: Option<Callback<(String, String, String, String)>>,
    sid_val: Option<String>,
) -> AnyView {
    let match_ids_signal = search_match_ids;
    let active_match_signal = active_search_match_id;

    let gs = groups.get();
    let start = vs.virtual_start.get();
    let end = vs.virtual_end.get().min(gs.len());
    let match_ids = match_ids_signal.map(|s| s.get()).unwrap_or_default();
    let active_match = active_match_signal.and_then(|s| s.get());
    let pending_id = pending_assistant_id.get();
    let cs = child_sessions.get();

    (start..end)
        .map(|i| {
            let group = gs[i].clone();
            let key = group.key.clone();
            let offset = vs.get_offset_for_index(i);
            let pending_clone = pending_id.clone().unwrap_or_default();

            view! {
                <div
                    class="message-turn-wrap"
                    data-group-key=key
                    data-index=i.to_string()
                    style:position="absolute"
                    style:top="0"
                    style:left="0"
                    style:width="100%"
                    style:transform=format!("translateY({}px)", offset)
                >
                    {render_turn_view(
                        group, Some(match_ids.clone()), active_match.clone(),
                        pending_clone, bm_cb, toggle_bm_cb.clone(), sid_val.clone(),
                        cs.clone(), subagent_messages,
                    )}
                </div>
            }
        })
        .collect_view()
        .into_any()
}
