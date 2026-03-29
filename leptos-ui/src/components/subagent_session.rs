//! SubagentSession — renders a subagent session's messages in a collapsible card.
//! Leptos port of `web-ui/src/SubagentSession.tsx`.
//!
//! Live sessions: messages arrive via SSE props.
//! Completed sessions: fetches messages from the REST API.
//!
//! Messages render with full markdown, code blocks, and tool call accordions
//! via `group_messages()` + `MessageTurn`, matching parent message rendering.

use leptos::prelude::*;
use crate::types::core::Message;
use crate::api::client::fetch_session_messages;
use crate::components::message_timeline::AccordionState;
use crate::components::message_turn::{group_messages, MessageTurn};
use web_sys::HtmlElement;

/// Renders a subagent session's messages inside a collapsible card.
///
/// `messages` is a reactive signal so that new messages update the body
/// **without** recreating the component (preserving scroll position and
/// accordion state).
#[component]
pub fn SubagentSession(
    #[prop(into)] session_id: String,
    #[prop(into)] title: String,
    messages: Signal<Vec<Message>>,
    #[prop(optional)] is_running: bool,
    #[prop(optional)] is_completed: bool,
    #[prop(optional)] is_error: bool,
    on_open_session: Option<Callback<String>>,
) -> impl IntoView {
    let has_sse_messages = messages.with_untracked(|m| !m.is_empty());

    // Read shared accordion state from context (survives parent re-renders).
    let accordion_ctx = use_context::<AccordionState>();

    // Determine expanded state: if user previously toggled, use that; else compute default.
    let auto_default = if !is_running && (is_completed || is_error) { false } else { is_running };
    let default_expanded = if let Some(AccordionState(map)) = accordion_ctx {
        let saved = map.with_untracked(|m| m.get(&session_id).copied());
        if let Some(val) = saved {
            val
        } else {
            // Seed the map so re-renders preserve this initial state
            map.update_untracked(|m| { m.insert(session_id.clone(), auto_default); });
            auto_default
        }
    } else {
        auto_default
    };

    let (expanded, set_expanded) = signal(default_expanded);
    let (fetched_messages, set_fetched_messages) = signal::<Option<Vec<Message>>>(None);
    let (is_fetching, set_is_fetching) = signal(false);
    let (fetch_attempted, set_fetch_attempted) = signal(false);

    let sid_for_toggle = session_id.clone();
    let handle_toggle = move |_: web_sys::MouseEvent| {
        let new_val = !expanded.get_untracked();
        set_expanded.set(new_val);
        // Persist in shared context so re-renders preserve user's choice
        if let Some(AccordionState(map)) = accordion_ctx {
            map.update(|m| { m.insert(sid_for_toggle.clone(), new_val); });
        }
    };

    // Fetch messages for completed tasks without SSE data
    let sid_for_fetch = session_id.clone();
    if !has_sse_messages && !is_running && (is_completed || is_error) && !fetch_attempted.get_untracked() {
        set_fetch_attempted.set(true);
        set_is_fetching.set(true);
        let sid = sid_for_fetch.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match fetch_session_messages(&sid, 100, None).await {
                Ok(resp) => {
                    if !resp.messages.is_empty() {
                        set_fetched_messages.set(Some(resp.messages));
                    }
                }
                Err(e) => {
                    log::warn!("SubagentSession: failed to fetch messages for {}: {}", sid, e);
                }
            }
            set_is_fetching.set(false);
        });
    }

    // No need for a separate display_messages clone — we read the
    // reactive `messages` signal (or `fetched_messages`) in the body.

    let session_id_for_open = session_id.clone();
    let session_id_display = session_id.clone();
    let scroll_ref = NodeRef::<leptos::html::Div>::new();

    // Reactive message count — drives auto-scroll effect below.
    let msg_count = Memo::new(move |_| messages.with(|m| m.len()));

    // Auto-scroll to bottom only if user was already near the bottom.
    // Prevents resetting user's scroll position when new messages arrive.
    Effect::new(move |prev_count: Option<usize>| {
        let current = msg_count.get();
        if let Some(prev) = prev_count {
            if current > prev {
                if let Some(el) = scroll_ref.get() {
                    let el: &HtmlElement = &el;
                    let distance = el.scroll_height() - el.scroll_top() - el.client_height();
                    // Only auto-scroll if within 100px of bottom (or first content)
                    if distance < 100 || el.scroll_top() == 0 {
                        el.set_scroll_top(el.scroll_height());
                    }
                }
            }
        }
        current
    });

    view! {
        <div class=move || {
            let mut cls = "subagent-session rounded-lg border overflow-hidden my-1".to_string();
            if is_running {
                cls.push_str(" border-primary/30 bg-primary/5");
            } else if is_error {
                cls.push_str(" border-error/20 bg-error/5");
            } else {
                cls.push_str(" border-border-subtle bg-bg-panel/20");
            }
            cls
        }>
            // Header
            <div class="subagent-header flex items-center justify-between">
                <button
                    class="subagent-toggle flex items-center gap-1.5 flex-1 px-3 py-1.5 text-left hover:bg-bg-element/20 transition-colors"
                    on:click=handle_toggle
                >
                    // Chevron
                    {move || if expanded.get() {
                        view! {
                            <svg class="w-3.5 h-3.5 text-text-muted shrink-0" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                                <path d="M4 6l4 4 4-4" />
                            </svg>
                        }.into_any()
                    } else {
                        view! {
                            <svg class="w-3.5 h-3.5 text-text-muted shrink-0" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                                <path d="M6 4l4 4-4 4" />
                            </svg>
                        }.into_any()
                    }}
                    // Bot icon
                    <svg class="w-3.5 h-3.5 text-primary/60 shrink-0" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                        <rect x="3" y="5" width="10" height="8" rx="2" />
                        <path d="M8 5V3M5 9h0M11 9h0" />
                    </svg>
                    // Title
                    <span class="text-xs font-medium text-text truncate">{title.clone()}</span>
                    // Status
                    <span class="ml-auto flex items-center gap-1 shrink-0">
                        {if is_running {
                            view! {
                                <span class="flex items-center gap-1 text-[10px] text-primary">
                                    <span class="w-1.5 h-1.5 rounded-full bg-primary animate-pulse" />
                                    "running"
                                </span>
                            }.into_any()
                        } else if is_completed {
                            view! {
                                <span class="flex items-center gap-1 text-[10px] text-success">
                                    <svg class="w-3 h-3" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                                        <circle cx="8" cy="8" r="6" />
                                        <path d="M5 8l2 2 4-4" />
                                    </svg>
                                    "completed"
                                </span>
                            }.into_any()
                        } else if is_error {
                            view! {
                                <span class="flex items-center gap-1 text-[10px] text-error">
                                    <svg class="w-3 h-3" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                                        <circle cx="8" cy="8" r="6" />
                                        <path d="M6 6l4 4M10 6l-4 4" />
                                    </svg>
                                    "failed"
                                </span>
                            }.into_any()
                        } else {
                            view! { <span /> }.into_any()
                        }}
                    </span>
                </button>
                // Open session link
                {on_open_session.as_ref().map(|cb| {
                    let cb = cb.clone();
                    let sid = session_id_for_open.clone();
                    view! {
                        <button
                            class="subagent-open-link flex items-center gap-1 px-2 py-1 text-[10px] text-text-muted hover:text-primary transition-colors"
                            on:click=move |_: web_sys::MouseEvent| {
                                cb.run(sid.clone());
                            }
                            title="Open this session"
                        >
                            <svg class="w-2.5 h-2.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                                <path d="M10 2h4v4M6 10L14 2M12 9v4a1 1 0 01-1 1H3a1 1 0 01-1-1V5a1 1 0 011-1h4" />
                            </svg>
                            "Open"
                        </button>
                    }
                })}
            </div>

            // Body (expanded)
            {move || expanded.get().then(|| {
                if is_fetching.get() {
                    view! {
                        <div class="subagent-empty flex items-center gap-2 px-3 py-4 justify-center text-xs text-text-muted">
                            <span class="w-3.5 h-3.5 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
                            "Loading task output..."
                        </div>
                    }.into_any()
                } else {
                    // Read SSE messages reactively, or fall back to fetched
                    let msgs = if has_sse_messages {
                        messages.get()
                    } else {
                        fetched_messages.get().unwrap_or_default()
                    };

                    if msgs.is_empty() {
                        if is_running {
                            view! {
                                <div class="subagent-empty flex items-center gap-2 px-3 py-4 justify-center text-xs text-text-muted">
                                    <span class="w-3.5 h-3.5 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
                                    "Subagent starting..."
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="subagent-empty flex items-center justify-center px-3 py-4 text-xs text-text-muted/60">
                                    "No task output available"
                                </div>
                            }.into_any()
                        }
                    } else {
                        // Full rich rendering: group messages and render each group
                        // via MessageTurn (markdown, code blocks, tool call accordions).
                        let groups = group_messages(&msgs);
                        view! {
                            <div node_ref=scroll_ref class="subagent-messages max-h-[400px] overflow-y-auto border-t border-border-subtle/50">
                                {groups.into_iter().map(|group| {
                                    view! {
                                        <MessageTurn
                                            group=group
                                            search_match_ids=None
                                            active_search_match_id=None
                                            session_id=None
                                            is_bookmarked=None
                                            on_toggle_bookmark=None
                                        />
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }
            })}
        </div>
    }
}
