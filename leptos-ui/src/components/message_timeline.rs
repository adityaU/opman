//! MessageTimeline — renders the list of chat messages grouped by role.
//! Matches React `message-timeline/MessageTimeline.tsx` core behavior.
//! Uses MessageTurn for rich rendering with code blocks, tool calls, subagents.
//!
//! Performance optimizations vs naive approach:
//!   - group_messages uses Memo, only re-runs when messages signal changes
//!   - Search signals are read in isolated closures (don't trigger group re-render)
//!   - Scroll-to-active-match effect matches React behavior
//!   - pendingAssistantId computed once as a Memo
//!   - Virtualization: for 40+ groups, uses absolute-positioned items with
//!     a simple viewport-based render window (matches React @tanstack/react-virtual behavior)

use leptos::prelude::*;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::message_turn::{group_messages, MessageTurn};
use crate::hooks::use_sse_state::{SessionStatus, SseState};
use crate::types::api::SessionInfo;

/// Shared accordion state — survives component re-creation across reactive re-renders.
/// Key = tool-call ID or subagent session ID, Value = user-toggled expanded state.
/// Provided via context by MessageTimeline, consumed by ToolCallView / SubagentSession.
#[derive(Clone, Copy)]
pub struct AccordionState(pub RwSignal<HashMap<String, bool>>);

/// React: VIRTUALIZE_THRESHOLD = 40 groups before switching to virtual list.
const VIRTUALIZE_THRESHOLD: usize = 40;

/// React: estimated size per group in pixels (used for initial layout before measurement).
const ESTIMATED_ROW_HEIGHT: f64 = 160.0;

/// React: overscan — extra items to render above/below the viewport.
const OVERSCAN: usize = 5;

/// Schedule a closure on the next animation frame.
fn request_animation_frame(f: impl FnOnce() + 'static) {
    let cb = Closure::once_into_js(f);
    if let Some(w) = web_sys::window() {
        let _ = w.request_animation_frame(cb.unchecked_ref());
    }
}

/// MessageTimeline component — scrollable message list with auto-scroll behavior.
/// React: The scroll container is `.message-timeline` (flex:1, overflow-y:auto).
/// Older messages load automatically when scrolled near top (no button).
/// Jump-to-bottom is a sticky button sibling of `.message-timeline-inner`.
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

    // Store optional callbacks for prop threading into MessageTurn
    let stored_send_prompt = StoredValue::new(on_send_prompt);
    let stored_is_bookmarked = StoredValue::new(is_bookmarked);
    let stored_on_toggle_bookmark = StoredValue::new(on_toggle_bookmark);
    let stored_session_id = StoredValue::new(session_id);

    let scroll_container_ref = NodeRef::<leptos::html::Div>::new();
    let (should_auto_scroll, set_should_auto_scroll) = signal(true);
    let (show_jump_to_bottom, set_show_jump_to_bottom) = signal(false);

    // Shared accordion expanded state — survives component re-renders.
    // Provided via context so ToolCallView / SubagentSession can read/write.
    provide_context(AccordionState(RwSignal::new(HashMap::new())));

    // Scroll direction detection state (React: cumulative delta + threshold algorithm)
    let (last_scroll_top, set_last_scroll_top) = signal(0i32);
    let (cumulative_delta, set_cumulative_delta) = signal(0i32);
    const SCROLL_DIRECTION_THRESHOLD: i32 = 20;
    let (programmatic_scroll, set_programmatic_scroll) = signal(false);

    // Throttle flag: prevents re-entrant / back-to-back update_virtual_range calls
    // within the same animation frame during scroll.
    let (scroll_raf_pending, set_scroll_raf_pending) = signal(false);

    // Virtualization state — tracks the visible range of groups to render
    let (virtual_start, set_virtual_start) = signal(0usize);
    let (virtual_end, set_virtual_end) = signal(0usize);
    // Map group index → measured height (populated after render via data-index)
    let measured_heights: StoredValue<HashMap<usize, f64>> = StoredValue::new(HashMap::new());

    // Store optional callbacks so they can be accessed from multiple closures
    let stored_scroll_dir = StoredValue::new(on_scroll_direction);
    let stored_load_older = StoredValue::new(on_load_older);

    // ── Grouped messages memo ──
    // React: groupMessages(messages, prevGroupsRef.current) with referential
    // stability for unchanged groups. We compute groups as a Memo so they only
    // re-derive when the messages signal actually changes.
    let groups = Memo::new(move |_| {
        let msgs = messages.get();
        group_messages(&msgs)
    });

    // Whether virtualization is active
    let use_virtual = Memo::new(move |_| groups.get().len() >= VIRTUALIZE_THRESHOLD);

    // ── pendingAssistantId — React computes this with useMemo ──
    // Find the last assistant message whose metadata.time.completed is falsy.
    let pending_assistant_id = Memo::new(move |_| {
        let msgs = messages.get();
        for msg in msgs.iter().rev() {
            if msg.info.role != "assistant" { continue; }
            // Check metadata.time.completed
            let completed = msg.metadata.as_ref().and_then(|md| {
                md.time.as_ref().and_then(|t| {
                    t.get("completed").and_then(|v| {
                        if v.is_null() || v == &serde_json::Value::Bool(false) || v == &serde_json::json!(0) {
                            None
                        } else {
                            Some(())
                        }
                    })
                })
            });
            if completed.is_none() {
                return Some(msg.info.effective_id());
            }
        }
        None
    });

    // ── Child sessions: sessions whose parentID == active session ──
    // React: childSessions = appState.projects[active].sessions.filter(s => s.parentID == activeSessionId)
    // Uses derived_active_project to avoid subscribing to the full app_state.
    let child_sessions = Memo::new(move |_| {
        let proj = match sse.derived_active_project.get() {
            Some(p) => p,
            None => return Vec::<SessionInfo>::new(),
        };
        let active_sid = match proj.active_session.as_deref() {
            Some(s) if !s.is_empty() => s,
            _ => return Vec::new(),
        };
        let mut children: Vec<SessionInfo> = proj
            .sessions
            .iter()
            .filter(|s| s.parent_id == active_sid)
            .cloned()
            .collect();
        // Sort by time.created (ascending) to match React
        children.sort_by(|a, b| {
            a.time
                .created
                .partial_cmp(&b.time.created)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        children
    });

    // ── Subagent messages signal (reactive read from SseState) ──
    let subagent_messages = sse.subagent_messages;

    // ── Search: message ID → group key map (for scroll-to-match) ──
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

    // ── Helper: get height of group at index (measured or estimated) ──
    let get_row_height = move |index: usize| -> f64 {
        measured_heights.with_value(|m| m.get(&index).copied().unwrap_or(ESTIMATED_ROW_HEIGHT))
    };

    // ── Helper: compute total virtual height ──
    let get_total_height = move |count: usize| -> f64 {
        let mut total = 0.0;
        for i in 0..count {
            total += get_row_height(i);
        }
        total
    };

    // ── Helper: compute the translateY offset for a given group index ──
    let get_offset_for_index = move |index: usize| -> f64 {
        let mut offset = 0.0;
        for i in 0..index {
            offset += get_row_height(i);
        }
        offset
    };

    // ── Virtualization: compute visible range from scroll position ──
    let update_virtual_range = move || {
        let count = groups.with_untracked(|g| g.len());
        if count < VIRTUALIZE_THRESHOLD {
            set_virtual_start.set(0);
            set_virtual_end.set(count);
            return;
        }

        let Some(el) = scroll_container_ref.get() else { return };
        let el: &web_sys::HtmlElement = &el;
        let scroll_top = el.scroll_top() as f64;
        let viewport_height = el.client_height() as f64;

        // Find start index: first group whose bottom edge is past scroll_top
        let mut offset = 0.0;
        let mut start = 0;
        for i in 0..count {
            let h = get_row_height(i);
            if offset + h > scroll_top {
                start = i;
                break;
            }
            offset += h;
            if i == count - 1 {
                start = count; // scrolled past all
            }
        }

        // Find end index: first group whose top edge is past scroll_top + viewport_height
        let mut end = start;
        let bottom = scroll_top + viewport_height;
        let mut off = offset;
        for i in start..count {
            if off >= bottom {
                end = i;
                break;
            }
            off += get_row_height(i);
            if i == count - 1 {
                end = count; // include last
            }
        }

        // Apply overscan
        let os_start = start.saturating_sub(OVERSCAN);
        let os_end = (end + OVERSCAN).min(count);

        set_virtual_start.set(os_start);
        set_virtual_end.set(os_end);
    };

    // ── Measure rendered items and update heights ──
    let measure_items = move || {
        let Some(container) = scroll_container_ref.get() else { return };
        let el: &web_sys::HtmlElement = &container;
        // Query all rendered virtual items
        if let Ok(nodes) = el.query_selector_all("[data-index]") {
            let mut changed = false;
            measured_heights.update_value(|map| {
                for i in 0..nodes.length() {
                    if let Some(node) = nodes.item(i) {
                        if let Ok(elem) = node.dyn_into::<web_sys::HtmlElement>() {
                            if let Some(idx_str) = elem.get_attribute("data-index") {
                                if let Ok(idx) = idx_str.parse::<usize>() {
                                    let h = elem.offset_height() as f64;
                                    if h > 0.0 {
                                        let prev = map.get(&idx).copied().unwrap_or(0.0);
                                        if (prev - h).abs() > 1.0 {
                                            map.insert(idx, h);
                                            changed = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
            if changed {
                // Re-compute range with updated measurements
                // (will be called on next scroll or rAF)
            }
        }
    };

    // ── Scroll to active search match (React: useEffect on activeSearchMatchId) ──
    if let Some(active_match_signal) = active_search_match_id {
        let container_ref = scroll_container_ref;
        let id_to_key = message_id_to_group_key;
        Effect::new(move |_| {
            let active_id = active_match_signal.get();
            let active_id = match active_id {
                Some(id) if !id.is_empty() => id,
                _ => return,
            };
            let map = id_to_key.get();
            let group_key = match map.get(&active_id) {
                Some(k) => k.clone(),
                None => return,
            };

            // If virtual, we may need to scroll the virtualizer first to
            // materialize the group in the DOM, then scrollIntoView.
            if use_virtual.get_untracked() {
                // Find the group index for this key
                let gs = groups.get_untracked();
                if let Some(group_idx) = gs.iter().position(|g| g.key == group_key) {
                    // Scroll container to roughly the right offset to materialize item
                    if let Some(container) = container_ref.get() {
                        let offset = get_offset_for_index(group_idx);
                        let el: &web_sys::HtmlElement = &container;
                        let viewport_h = el.client_height() as f64;
                        // Center the target
                        let target_scroll = (offset - viewport_h / 2.0).max(0.0);
                        set_programmatic_scroll.set(true);
                        el.set_scroll_top(target_scroll as i32);
                        update_virtual_range();
                    }
                }
            }

            // After potential range update, find the element and smooth scroll
            // Use rAF to wait for DOM update
            let container_ref2 = container_ref;
            let gk = group_key.clone();
            leptos::task::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(16).await; // ~1 frame
                if let Some(container) = container_ref2.get() {
                    let el: &web_sys::HtmlElement = &container;
                    let selector = format!("[data-group-key=\"{}\"]", gk.replace('"', "\\\""));
                    if let Ok(Some(target)) = el.query_selector(&selector) {
                        let opts = js_sys::Object::new();
                        let _ = js_sys::Reflect::set(&opts, &"behavior".into(), &"smooth".into());
                        let _ = js_sys::Reflect::set(&opts, &"block".into(), &"center".into());
                        let scroll_fn = js_sys::Reflect::get(&target, &"scrollIntoView".into()).ok();
                        if let Some(func) = scroll_fn {
                            if let Ok(func) = func.dyn_into::<js_sys::Function>() {
                                let _ = func.call1(&target, &opts);
                            }
                        }
                    }
                }
            });
        });
    }

    // ── Content fingerprint for streaming auto-scroll (React pattern) ──
    let content_fingerprint = Memo::new(move |_| {
        let msgs = messages.get();
        msgs.last().map(|m| m.parts.len()).unwrap_or(0)
    });

    // Auto-scroll to bottom when messages change and auto-scroll is enabled
    Effect::new(move |_| {
        let _len = messages.with(|m| m.len()); // Subscribe to message changes
        let _status = session_status.get();
        let _fp = content_fingerprint.get(); // React: contentFingerprint dep

        if should_auto_scroll.get_untracked() {
            if let Some(el) = scroll_container_ref.get() {
                set_programmatic_scroll.set(true);
                let el: &web_sys::HtmlElement = &el;
                el.set_scroll_top(el.scroll_height());
                // After scrolling, update virtual range
                if use_virtual.get_untracked() {
                    update_virtual_range();
                }
            }
        }
    });

    // Scroll handler: auto-scroll tracking + direction detection + load-older + virtualization (React behavior)
    let on_scroll = move |_: web_sys::Event| {
        if let Some(el) = scroll_container_ref.get() {
            let el: &web_sys::HtmlElement = &el;
            let current_scroll_top = el.scroll_top();
            let scroll_height = el.scroll_height();
            let client_height = el.client_height();
            let distance_from_bottom = scroll_height - current_scroll_top - client_height;

            let delta = current_scroll_top - last_scroll_top.get_untracked();
            set_last_scroll_top.set(current_scroll_top);

            // Auto-scroll if within 100px of bottom
            let near_bottom = distance_from_bottom < 100;

            // Throttled virtualization range update via rAF — avoid layout
            // thrashing by deferring the range recalculation.  measure_items()
            // is NOT called here; it runs only after content changes (render
            // effects / rAF after group list changes).
            if use_virtual.get_untracked() && !scroll_raf_pending.get_untracked() {
                set_scroll_raf_pending.set(true);
                request_animation_frame(move || {
                    update_virtual_range();
                    set_scroll_raf_pending.set(false);
                });
            }

            // Programmatic scrolls (auto-scroll) — update UI state but skip
            // user-intent logic (direction detection, load-older, auto-scroll toggle)
            if programmatic_scroll.get_untracked() {
                set_programmatic_scroll.set(false);
                set_cumulative_delta.set(0);
                set_show_jump_to_bottom.set(!near_bottom && scroll_height > client_height + 200);
                return;
            }

            // React: if (delta < -5) shouldAutoScrollRef.current = false
            if delta < -5 {
                set_should_auto_scroll.set(false);
            }
            if near_bottom {
                set_should_auto_scroll.set(true);
            }
            set_show_jump_to_bottom.set(!should_auto_scroll.get_untracked() && !near_bottom);

            // Scroll direction detection (React: mobile dock collapse/expand)
            stored_scroll_dir.with_value(|dir_opt| {
                if let Some(ref dir_cb) = dir_opt {
                    let cum = cumulative_delta.get_untracked();
                    // Reset on direction reversal
                    if (cum > 0 && delta < 0) || (cum < 0 && delta > 0) {
                        set_cumulative_delta.set(0);
                    }
                    let new_cum = cumulative_delta.get_untracked() + delta;
                    set_cumulative_delta.set(new_cum);
                    if new_cum.abs() >= SCROLL_DIRECTION_THRESHOLD {
                        let direction = if new_cum < 0 { "up" } else { "down" };
                        set_cumulative_delta.set(0);
                        dir_cb.run(direction.to_string());
                    }
                }
            });

            // React: Load older messages when scrolled within 200px of top
            if current_scroll_top < 200 && has_older.get_untracked() && !is_loading_older.get_untracked() {
                let handled = stored_load_older.with_value(|lo_opt| {
                    if let Some(ref cb) = lo_opt {
                        cb.run(());
                        true
                    } else {
                        false
                    }
                });
                if !handled {
                    // Preserve scroll position after loading older messages
                    let prev_scroll_height = scroll_height;
                    sse.load_older_messages();
                    // After load, adjust scroll to maintain position
                    let el_clone = el.clone();
                    leptos::task::spawn_local(async move {
                        // Small delay to let DOM update
                        gloo_timers::future::TimeoutFuture::new(50).await;
                        let new_scroll_height = el_clone.scroll_height();
                        let height_diff = new_scroll_height - prev_scroll_height;
                        if height_diff > 0 {
                            el_clone.set_scroll_top(current_scroll_top + height_diff);
                        }
                    });
                }
            }
        }
    };

    let scroll_to_bottom = move |_: web_sys::MouseEvent| {
        if let Some(el) = scroll_container_ref.get() {
            set_programmatic_scroll.set(true);
            let el: &web_sys::HtmlElement = &el;
            el.set_scroll_top(el.scroll_height());
        }
        set_should_auto_scroll.set(true);
        set_show_jump_to_bottom.set(false);
    };

    // ── Stable branch signal ──
    // Avoids re-running the massive render closure when only message *content*
    // changes but the branch (welcome / shimmer / empty / normal) stays the same.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TimelineBranch {
        Welcome,
        Loading,
        EmptyIdle,
        Normal,
    }
    let branch = Memo::new(move |_| {
        if tracked_sid.get().is_none() {
            return TimelineBranch::Welcome;
        }
        let msgs_empty = messages.with(|m| m.is_empty());
        if is_loading.get() && msgs_empty {
            return TimelineBranch::Loading;
        }
        if msgs_empty && session_status.get() == SessionStatus::Idle {
            return TimelineBranch::EmptyIdle;
        }
        TimelineBranch::Normal
    });

    view! {
        // Conditional rendering based on state
        {move || {
            match branch.get() {
                TimelineBranch::Welcome => {
                    // No session selected — welcome empty state (React path A)
                    return view! {
                        <WelcomeEmpty />
                    }.into_any();
                }
                TimelineBranch::Loading => {
                    // Loading shimmer (React path B)
                    return view! {
                        <div class="message-timeline" role="log" aria-live="polite" aria-label="Chat messages">
                            <div class="message-timeline-inner">
                                <MessageShimmer />
                            </div>
                        </div>
                    }.into_any();
                }
                TimelineBranch::EmptyIdle => {
                    // Empty session (React path C — only when idle, matching React)
                    return stored_send_prompt.with_value(|send_cb| {
                        if let Some(cb) = send_cb.clone() {
                            view! {
                                <NewSessionEmpty sse=sse on_send_prompt=cb />
                            }.into_any()
                        } else {
                            view! {
                                <NewSessionEmpty sse=sse />
                            }.into_any()
                        }
                    });
                }
                TimelineBranch::Normal => {}
            }

            // ── Normal message list (with optional virtualization) ──
            let is_virtual = use_virtual.get();
            let current_groups = groups.get();
            let group_count = current_groups.len();

            // For non-virtual path, read search/bookmark props once
            let match_ids_signal = search_match_ids;
            let active_match_signal = active_search_match_id;

            // Extract bookmark/session props from stored values (Option types)
            let bm_cb: Option<Callback<String, bool>> = stored_is_bookmarked.get_value();
            let toggle_bm_cb: Option<Callback<(String, String, String, String)>> = stored_on_toggle_bookmark.get_value();
            let sid_val: Option<String> = stored_session_id.get_value().and_then(|m| m.get_untracked());

            if is_virtual {
                // ── Virtual rendering path ──
                // Initialize virtual range on first render
                update_virtual_range();

                // Schedule measurement after first paint
                leptos::task::spawn_local(async move {
                    gloo_timers::future::TimeoutFuture::new(32).await;
                    measure_items();
                    update_virtual_range();
                });

                let total_height = get_total_height(group_count);

                view! {
                    <div
                        node_ref=scroll_container_ref
                        class="message-timeline"
                        role="log"
                        aria-live="polite"
                        aria-label="Chat messages"
                        on:scroll=on_scroll
                    >
                        <div
                            class="message-timeline-inner"
                            style:height=format!("{}px", total_height)
                            style:position="relative"
                        >
                            // Loading older indicator
                            {move || {
                                if is_loading_older.get() {
                                    Some(view! {
                                        <div class="load-older-messages" style="position: relative; z-index: 1;">
                                            <span class="load-older-spinner">"Loading older messages..."</span>
                                        </div>
                                    })
                                } else {
                                    None
                                }
                            }}

                            // Virtual items — only render groups in [virtual_start, virtual_end)
                            {move || {
                                let gs = groups.get();
                                let start = virtual_start.get();
                                let end = virtual_end.get().min(gs.len());
                                let match_ids = match_ids_signal.map(|s| s.get()).unwrap_or_default();
                                let active_match = active_match_signal.map(|s| s.get()).unwrap_or_default();
                                let pending_id = pending_assistant_id.get();
                                let cs = child_sessions.get();
                                let sub_msgs = subagent_messages.get();

                                (start..end).map(|i| {
                                    let group = gs[i].clone();
                                    let key = group.key.clone();
                                    let offset = get_offset_for_index(i);
                                    let match_ids_clone = Some(match_ids.clone());
                                    let active_match_clone = active_match.clone();
                                    let pending_clone = pending_id.clone().unwrap_or_default();
                                    let bm = bm_cb;
                                    let toggle_bm = toggle_bm_cb.clone();
                                    let sid = sid_val.clone();
                                    let cs_clone = cs.clone();
                                    let sub_clone = sub_msgs.clone();

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
                                            <MessageTurn
                                                group=group
                                                child_sessions=cs_clone
                                                subagent_messages=sub_clone
                                                search_match_ids=match_ids_clone
                                                active_search_match_id={active_match_clone}
                                                pending_assistant_id={pending_clone}
                                                is_bookmarked=bm
                                                on_toggle_bookmark=toggle_bm
                                                session_id=sid
                                            />
                                        </div>
                                    }
                                }).collect_view()
                            }}
                        </div>

                        // Jump-to-bottom button
                        {move || {
                            if show_jump_to_bottom.get() {
                                Some(view! {
                                    <button
                                        class="jump-to-bottom"
                                        on:click=scroll_to_bottom
                                    >
                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                            <path d="M12 5v14M19 12l-7 7-7-7"/>
                                        </svg>
                                        <span>"Jump to bottom"</span>
                                    </button>
                                })
                            } else {
                                None
                            }
                        }}
                    </div>
                }.into_any()
            } else {
                // ── Non-virtual rendering path (< 40 groups) ──
                view! {
                    <div
                        node_ref=scroll_container_ref
                        class="message-timeline"
                        role="log"
                        aria-live="polite"
                        aria-label="Chat messages"
                        on:scroll=on_scroll
                    >
                        <div class="message-timeline-inner">
                            // Loading older indicator
                            {move || {
                                if is_loading_older.get() {
                                    Some(view! {
                                        <div class="load-older-messages">
                                            <span class="load-older-spinner">"Loading older messages..."</span>
                                        </div>
                                    })
                                } else {
                                    None
                                }
                            }}

                            // Grouped message turns — all rendered
                            {move || {
                                let gs = groups.get();
                                let match_ids = match_ids_signal.map(|s| s.get()).unwrap_or_default();
                                let active_match = active_match_signal.map(|s| s.get()).unwrap_or_default();
                                let pending_id = pending_assistant_id.get();
                                let cs = child_sessions.get();
                                let sub_msgs = subagent_messages.get();

                                gs.into_iter().map(|group| {
                                    let match_ids_clone = Some(match_ids.clone());
                                    let active_match_clone = active_match.clone();
                                    let key = group.key.clone();
                                    let pending_clone = pending_id.clone().unwrap_or_default();
                                    let bm = bm_cb;
                                    let toggle_bm = toggle_bm_cb.clone();
                                    let sid = sid_val.clone();
                                    let cs_clone = cs.clone();
                                    let sub_clone = sub_msgs.clone();
                                    view! {
                                        <div class="message-turn-wrap" data-group-key=key>
                                            <MessageTurn
                                                group=group
                                                child_sessions=cs_clone
                                                subagent_messages=sub_clone
                                                search_match_ids=match_ids_clone
                                                active_search_match_id={active_match_clone}
                                                pending_assistant_id={pending_clone}
                                                is_bookmarked=bm
                                                on_toggle_bookmark=toggle_bm
                                                session_id=sid
                                            />
                                        </div>
                                    }
                                }).collect_view()
                            }}
                        </div>

                        // Jump-to-bottom button
                        {move || {
                            if show_jump_to_bottom.get() {
                                Some(view! {
                                    <button
                                        class="jump-to-bottom"
                                        on:click=scroll_to_bottom
                                    >
                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                            <path d="M12 5v14M19 12l-7 7-7-7"/>
                                        </svg>
                                        <span>"Jump to bottom"</span>
                                    </button>
                                })
                            } else {
                                None
                            }
                        }}
                    </div>
                }.into_any()
            }
        }}
    }
}

/// Shimmer loading placeholder — matches React `MessageShimmer`.
/// Uses semantic shimmer-* class names that map to the same CSS as React.
#[component]
fn MessageShimmer() -> impl IntoView {
    view! {
        <div class="message-shimmer" aria-label="Loading messages">
            <div class="shimmer-turn shimmer-user">
                <div class="shimmer-content">
                    <div class="shimmer-header-row">
                        <div class="shimmer-avatar" />
                        <div class="shimmer-line shimmer-role-label" />
                    </div>
                    <div class="shimmer-line shimmer-w-55" />
                    <div class="shimmer-line shimmer-w-35" />
                </div>
            </div>
            <div class="shimmer-turn shimmer-assistant">
                <div class="shimmer-content">
                    <div class="shimmer-header-row">
                        <div class="shimmer-avatar" />
                        <div class="shimmer-line shimmer-role-label" />
                    </div>
                    <div class="shimmer-line shimmer-w-90" />
                    <div class="shimmer-line shimmer-w-75" />
                    <div class="shimmer-line shimmer-w-60" />
                    <div class="shimmer-line shimmer-w-45" />
                </div>
            </div>
            <div class="shimmer-turn shimmer-user">
                <div class="shimmer-content">
                    <div class="shimmer-header-row">
                        <div class="shimmer-avatar" />
                        <div class="shimmer-line shimmer-role-label" />
                    </div>
                    <div class="shimmer-line shimmer-w-40" />
                </div>
            </div>
            <div class="shimmer-turn shimmer-assistant">
                <div class="shimmer-content">
                    <div class="shimmer-header-row">
                        <div class="shimmer-avatar" />
                        <div class="shimmer-line shimmer-role-label" />
                    </div>
                    <div class="shimmer-line shimmer-w-80" />
                    <div class="shimmer-line shimmer-w-65" />
                    <div class="shimmer-line shimmer-w-50" />
                </div>
            </div>
        </div>
    }
}

/// Welcome empty state — matches React `WelcomeEmpty` exactly.
/// Shown when no session is selected.
#[component]
fn WelcomeEmpty() -> impl IntoView {
    view! {
        <div class="message-timeline-empty">
            <div class="message-timeline-welcome">
                <h2>"Welcome to OpenCode"</h2>
                <p>"Select a session from the sidebar or create a new one to start chatting."</p>
                <div class="message-timeline-shortcuts">
                    <kbd>"Cmd+Shift+N"</kbd>" New Session"
                    <kbd>"Cmd+Shift+P"</kbd>" Command Palette"
                    <kbd>"Cmd'"</kbd>" Model Picker"
                </div>
            </div>
        </div>
    }
}

/// New session empty state — matches React `NewSessionEmpty` exactly.
/// Shown when session exists but has no messages and status is idle.
#[component]
fn NewSessionEmpty(
    sse: SseState,
    #[prop(optional)] on_send_prompt: Option<Callback<String>>,
) -> impl IntoView {
    let session_directory = Memo::new(move |_| {
        let path = sse.active_project()
            .map(|p| p.path.clone())
            .unwrap_or_default();
        if path.is_empty() { None } else { Some(path) }
    });

    // Store send callback for use in button closures
    let stored_send = StoredValue::new(on_send_prompt);

    // Example prompts matching React EXAMPLE_PROMPTS
    let prompts: &[(&str, &str)] = &[
        ("code", "Refactor the auth module to use JWT tokens"),
        ("bug", "Find and fix the memory leak in the worker pool"),
        ("lightbulb", "Add unit tests for the API endpoints"),
        ("message", "Explain the architecture of this project"),
    ];

    let prompt_views = prompts.iter().map(|(icon, text)| {
        let text_owned = text.to_string();
        let text_for_click = text.to_string();
        let icon_svg = match *icon {
            "code" => view! {
                <svg class="new-session-prompt-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="16 18 22 12 16 6" /><polyline points="8 6 2 12 8 18" />
                </svg>
            }.into_any(),
            "bug" => view! {
                <svg class="new-session-prompt-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="m8 2 1.88 1.88M14.12 3.88 16 2M9 7.13v-1a3.003 3.003 0 1 1 6 0v1" />
                    <path d="M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6" />
                    <path d="M12 20v-9M6.53 9C4.6 8.8 3 7.1 3 5M6 13H2M3 21c0-2.1 1.7-3.9 3.8-4M20.97 5c0 2.1-1.6 3.8-3.5 4M22 13h-4M17.2 17c2.1.1 3.8 1.9 3.8 4" />
                </svg>
            }.into_any(),
            "lightbulb" => view! {
                <svg class="new-session-prompt-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5" />
                    <path d="M9 18h6M10 22h4" />
                </svg>
            }.into_any(),
            _ => view! {
                <svg class="new-session-prompt-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
                </svg>
            }.into_any(),
        };
        view! {
            <button
                class="new-session-prompt-card"
                on:click=move |_| {
                    stored_send.with_value(|cb| {
                        if let Some(ref cb) = cb {
                            cb.run(text_for_click.clone());
                        }
                    });
                }
            >
                {icon_svg}
                <span>{text_owned}</span>
            </button>
        }
    }).collect_view();

    view! {
        <div class="message-timeline-empty">
            <div class="message-timeline-welcome new-session-welcome">
                <h2>"New Session"</h2>

                <div class="new-session-info">
                    {move || {
                        let dir: Option<String> = session_directory.get();
                        dir.map(|d| {
                            let d2 = d.clone();
                            view! {
                                <div class="new-session-info-row">
                                    // FolderOpen icon
                                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2" />
                                    </svg>
                                    <span class="new-session-directory" title=d2>{d}</span>
                                </div>
                            }
                        })
                    }}
                </div>

                <p>"Type a message below or try one of these:"</p>

                <div class="new-session-prompts">
                    {prompt_views}
                </div>

                <div class="message-timeline-shortcuts">
                    <kbd>"Cmd'"</kbd>" Model Picker"
                    <kbd>"Cmd+Shift+E"</kbd>" Editor"
                    <kbd>"Cmd+Shift+G"</kbd>" Git"
                </div>
            </div>
        </div>
    }
}
