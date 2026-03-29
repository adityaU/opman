//! MessageTimeline — scrollable chat message list with keyed rendering,
//! virtualization (40+ groups), and search-match scrolling.

pub mod types;
pub mod empty_states;
pub mod render;
pub mod scroll;
pub mod virtual_list;

pub use types::AccordionState;

use leptos::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::components::message_turn::group_messages;
use crate::hooks::use_sse_state::{SessionStatus, SseState};
use crate::types::api::SessionInfo;

use empty_states::{MessageShimmer, NewSessionEmpty, WelcomeEmpty};
use render::{render_keyed_items, render_virtual_items};
use scroll::{setup_search_scroll_effect, ScrollState};
use types::{request_animation_frame, VIRTUALIZE_THRESHOLD};
use virtual_list::VirtualState;

/// MessageTimeline component — scrollable message list with auto-scroll behavior.
#[component]
pub fn MessageTimeline(
    sse: SseState,
    #[prop(optional)] on_load_older: Option<Callback<()>>,
    #[prop(optional)] on_scroll_direction: Option<Callback<String>>,
    #[prop(optional)] search_match_ids: Option<ReadSignal<HashSet<String>>>,
    #[prop(optional)] active_search_match_id: Option<ReadSignal<Option<String>>>,
    #[prop(optional)] on_send_prompt: Option<Callback<String>>,
    #[prop(optional)] is_bookmarked: Option<Callback<String, bool>>,
    #[prop(optional)] on_toggle_bookmark: Option<Callback<(String, String, String, String)>>,
    #[prop(optional)] session_id: Option<Memo<Option<String>>>,
) -> impl IntoView {
    let messages = sse.messages;
    let is_loading = sse.is_loading_messages;
    let has_older = sse.has_older_messages;
    let is_loading_older = sse.is_loading_older;
    let session_status = sse.session_status;
    let tracked_sid = Memo::new(move |_| sse.tracked_session_id_reactive());

    let stored_send_prompt = StoredValue::new(on_send_prompt);
    let stored_is_bookmarked = StoredValue::new(is_bookmarked);
    let stored_on_toggle_bookmark = StoredValue::new(on_toggle_bookmark);
    let stored_session_id = StoredValue::new(session_id);
    let stored_scroll_dir = StoredValue::new(on_scroll_direction);
    let stored_load_older = StoredValue::new(on_load_older);

    // Shared accordion state
    provide_context(AccordionState(RwSignal::new(HashMap::new())));
    let ss = ScrollState::new();
    let vs = VirtualState::new();

    let groups = Memo::new(move |_| group_messages(&messages.get()));
    let use_virtual = Memo::new(move |_| groups.get().len() >= VIRTUALIZE_THRESHOLD);

    let pending_assistant_id = Memo::new(move |_| {
        let msgs = messages.get();
        for msg in msgs.iter().rev() {
            if msg.info.role != "assistant" { continue; }
            let completed = msg.metadata.as_ref().and_then(|md| {
                md.time.as_ref().and_then(|t| {
                    t.get("completed").and_then(|v| {
                        if v.is_null()
                            || v == &serde_json::Value::Bool(false)
                            || v == &serde_json::json!(0)
                        { None } else { Some(()) }
                    })
                })
            });
            if completed.is_none() {
                return Some(msg.info.effective_id());
            }
        }
        None
    });

    let child_sessions = Memo::new(move |_| {
        let proj = match sse.derived_active_project.get() {
            Some(p) => p,
            None => return Vec::<SessionInfo>::new(),
        };
        let active_sid = match proj.active_session.as_deref() {
            Some(s) if !s.is_empty() => s,
            _ => return Vec::new(),
        };
        let mut children: Vec<SessionInfo> = proj.sessions.iter()
            .filter(|s| s.parent_id == active_sid)
            .cloned().collect();
        children.sort_by(|a, b| {
            a.time.created.partial_cmp(&b.time.created).unwrap_or(std::cmp::Ordering::Equal)
        });
        children
    });

    let subagent_messages = sse.subagent_messages;

    let message_id_to_group_key = Memo::new(move |_| {
        let gs = groups.get();
        let mut map = HashMap::new();
        for group in &gs {
            for msg in &group.messages {
                let id = msg.info.effective_id();
                if !id.is_empty() {
                    map.insert(id, group.key.clone());
                }
            }
        }
        map
    });

    let update_virtual_range = move || { vs.update_range(groups, ss.scroll_container_ref); };
    let measure_items = move || { vs.measure_items(ss.scroll_container_ref); };
    let get_offset_for_index = move |i: usize| -> f64 { vs.get_offset_for_index(i) };

    setup_search_scroll_effect(
        active_search_match_id, message_id_to_group_key,
        use_virtual, groups, ss, get_offset_for_index, update_virtual_range);

    Effect::new(move |_| { let _sid = tracked_sid.get(); ss.reset(); });

    // Scroll to bottom after loading
    Effect::new(move |_| {
        if is_loading.get() || !ss.should_auto_scroll.get_untracked() { return; }
        request_animation_frame(move || {
            request_animation_frame(move || { ss.scroll_to_bottom(); });
        });
    });

    let content_fingerprint = Memo::new(move |_| {
        messages.get().last().map(|m| m.parts.len()).unwrap_or(0)
    });

    // Auto-scroll on message/status change
    Effect::new(move |_| {
        let _len = messages.with(|m| m.len());
        let _status = session_status.get();
        let _fp = content_fingerprint.get();
        if ss.should_auto_scroll.get_untracked() {
            ss.scroll_to_bottom();
            if use_virtual.get_untracked() { update_virtual_range(); }
        }
    });

    // Build scroll handler
    let on_scroll = ss.build_on_scroll(
        use_virtual, has_older, is_loading_older,
        stored_scroll_dir, stored_load_older, sse, update_virtual_range);

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Branch { Welcome, Loading, EmptyIdle, Normal }

    let branch = Memo::new(move |_| {
        if tracked_sid.get().is_none() { return Branch::Welcome; }
        let msgs_empty = messages.with(|m| m.is_empty());
        if is_loading.get() && msgs_empty { return Branch::Loading; }
        if msgs_empty && session_status.get() == SessionStatus::Idle {
            return Branch::EmptyIdle;
        }
        Branch::Normal
    });

    let stable_groups = Memo::new(move |_| {
        let gs = groups.get();
        if gs.len() <= 1 { Vec::new() } else { gs[..gs.len() - 1].to_vec() }
    });
    let last_group = Memo::new(move |_| groups.get().last().cloned());

    view! {
        {move || match branch.get() {
            Branch::Welcome => view! { <WelcomeEmpty /> }.into_any(),
            Branch::Loading => view! {
                <div class="message-timeline" role="log" aria-live="polite"
                     aria-label="Chat messages">
                    <div class="message-timeline-inner"><MessageShimmer /></div>
                </div>
            }.into_any(),
            Branch::EmptyIdle => stored_send_prompt.with_value(|send_cb| {
                if let Some(cb) = send_cb.clone() {
                    view! { <NewSessionEmpty sse=sse on_send_prompt=cb /> }.into_any()
                } else {
                    view! { <NewSessionEmpty sse=sse /> }.into_any()
                }
            }),
            Branch::Normal => view! { <div /> }.into_any(),
        }}

        // Normal message list — scroll container lives here so on_scroll
        // is moved once and the closure stays FnMut-compatible.
        <div
            node_ref=ss.scroll_container_ref
            class="message-timeline"
            role="log"
            aria-live="polite"
            aria-label="Chat messages"
            on:scroll=on_scroll
            style:display=move || {
                if branch.get() == Branch::Normal { "" } else { "none" }
            }
        >
            <div class="message-timeline-inner"
                 style:height=move || {
                     if !use_virtual.get() { return String::new(); }
                     let count = groups.with(|g| g.len());
                     format!("{}px", vs.get_total_height(count))
                 }
                 style:position=move || {
                     if use_virtual.get() { "relative" } else { "" }
                 }
            >
                // Loading older indicator
                {move || {
                    is_loading_older.get().then(|| view! {
                        <div class="load-older-messages">
                            <span class="load-older-spinner">"Loading older messages..."</span>
                        </div>
                    })
                }}

                // Content: keyed or virtual
                {move || {
                    let bm_cb = stored_is_bookmarked.get_value();
                    let toggle_bm_cb = stored_on_toggle_bookmark.get_value();
                    let sid_val = stored_session_id.get_value()
                        .and_then(|m| m.get_untracked());

                    if use_virtual.get() {
                        update_virtual_range();
                        leptos::task::spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(32).await;
                            measure_items();
                            update_virtual_range();
                        });
                        render_virtual_items(
                            vs, groups, search_match_ids, active_search_match_id,
                            pending_assistant_id, child_sessions, subagent_messages,
                            bm_cb, toggle_bm_cb, sid_val,
                        )
                    } else {
                        render_keyed_items(
                            stable_groups, last_group,
                            search_match_ids, active_search_match_id,
                            pending_assistant_id, child_sessions, subagent_messages,
                            bm_cb, toggle_bm_cb, sid_val,
                        )
                    }
                }}
            </div>

            // Jump-to-bottom button
            {move || {
                ss.show_jump_to_bottom.get().then(|| {
                    let click = move |_: web_sys::MouseEvent| {
                        ss.scroll_to_bottom();
                        ss.set_should_auto_scroll.set(true);
                        ss.set_show_jump_to_bottom.set(false);
                    };
                    view! {
                        <button class="jump-to-bottom" on:click=click>
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none"
                                stroke="currentColor" stroke-width="2"
                                stroke-linecap="round" stroke-linejoin="round">
                                <path d="M12 5v14M19 12l-7 7-7-7"/>
                            </svg>
                            <span>"Jump to bottom"</span>
                        </button>
                    }
                })
            }}
        </div>
    }
}
