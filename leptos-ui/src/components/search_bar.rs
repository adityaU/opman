//! SearchBar — in-session search with regex toggle, match navigation.

use crate::types::core::Message;
use leptos::prelude::*;
use std::collections::HashSet;

/// Search match info.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    pub message_id: String,
    pub group_index: usize,
    pub snippet: String,
}

/// Extract all searchable text from a message.
fn get_searchable_text(msg: &Message) -> String {
    let mut parts = Vec::new();
    for part in &msg.parts {
        if let Some(ref text) = part.text {
            parts.push(text.clone());
        }
        if let Some(ref tool) = part.tool {
            parts.push(tool.clone());
        }
        if let Some(ref tn) = part.tool_name {
            parts.push(tn.clone());
        }
        if let Some(ref args) = part.args {
            parts.push(serde_json::to_string(args).unwrap_or_default());
        }
        if let Some(ref result) = part.result {
            if let Some(s) = result.as_str() {
                parts.push(s.to_string());
            } else {
                parts.push(serde_json::to_string(result).unwrap_or_default());
            }
        }
        if let Some(ref state) = part.state {
            if let Some(ref output) = state.output {
                parts.push(output.clone());
            }
            if let Some(ref input) = state.input {
                if let Some(s) = input.as_str() {
                    parts.push(s.to_string());
                } else {
                    parts.push(serde_json::to_string(input).unwrap_or_default());
                }
            }
        }
    }
    parts.join(" ")
}

/// Build a group index map: message_id -> group_index.
fn build_group_index_map(messages: &[Message]) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    let mut group_idx: usize = 0;
    let mut last_role: Option<String> = None;

    for msg in messages {
        if last_role.as_ref() != Some(&msg.info.role) {
            if last_role.is_some() {
                group_idx += 1;
            }
            last_role = Some(msg.info.role.clone());
        }
        let id = msg.info.effective_id();
        if !id.is_empty() {
            map.insert(id, group_idx);
        }
    }
    map
}

/// In-session search bar.
#[component]
pub fn SearchBar(
    messages: Vec<Message>,
    on_close: Callback<()>,
    on_matches_changed: Callback<(HashSet<String>, Option<String>)>,
    #[prop(optional)] on_scroll_to_group: Option<Callback<usize>>,
) -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (use_regex, set_use_regex) = signal(false);
    let (active_index, set_active_index) = signal(0usize);

    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Focus input on mount
    Effect::new(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    // Build group index map
    let group_index_map = build_group_index_map(&messages);
    let messages_for_search = messages.clone();

    // Compute matches
    let matches = Memo::new(move |_| {
        let q = query.get();
        if q.trim().is_empty() {
            return Vec::new();
        }

        let use_re = use_regex.get();
        let lq = q.to_lowercase();

        // Build test function
        let test_fn: Box<dyn Fn(&str) -> bool> = if use_re {
            // Use JS RegExp instead of Rust regex crate to save ~500KB WASM size
            let re = js_sys::RegExp::new(&q, "i");
            Box::new(move |text: &str| re.test(text))
        } else {
            Box::new(move |text: &str| text.to_lowercase().contains(&lq))
        };

        let mut result = Vec::new();
        for msg in &messages_for_search {
            let text = get_searchable_text(msg);
            if test_fn(&text) {
                let id = msg.info.effective_id();
                let lower_text = text.to_lowercase();
                let idx = lower_text.find(&q.to_lowercase()).unwrap_or(0);
                let start = idx.saturating_sub(30);
                let end = (idx + q.len() + 30).min(text.len());
                let mut snippet = String::new();
                if start > 0 {
                    snippet.push_str("...");
                }
                snippet.push_str(&text[start..end]);
                if end < text.len() {
                    snippet.push_str("...");
                }

                result.push(SearchMatch {
                    message_id: id.clone(),
                    group_index: group_index_map.get(&id).copied().unwrap_or(0),
                    snippet,
                });
            }
        }
        result
    });

    // Notify parent of match changes
    Effect::new(move |_| {
        let m = matches.get();
        let idx = active_index.get();
        let match_ids: HashSet<String> = m.iter().map(|mm| mm.message_id.clone()).collect();
        let active_id = m.get(idx).map(|mm| mm.message_id.clone());
        on_matches_changed.run((match_ids, active_id));
    });

    // Reset active index when query changes
    Effect::new(move |_| {
        let _ = query.get();
        let _ = use_regex.get();
        set_active_index.set(0);
    });

    // Navigate to active match
    Effect::new(move |_| {
        let m = matches.get();
        let idx = active_index.get();
        if !m.is_empty() {
            if let Some(ref cb) = on_scroll_to_group {
                if let Some(match_item) = m.get(idx) {
                    cb.run(match_item.group_index);
                }
            }
        }
    });

    let go_next = move || {
        let len = matches.get_untracked().len();
        if len > 0 {
            set_active_index.update(|i| *i = (*i + 1) % len);
        }
    };

    let go_prev = move || {
        let len = matches.get_untracked().len();
        if len > 0 {
            set_active_index.update(|i| *i = (*i + len - 1) % len);
        }
    };

    let on_close_esc = on_close.clone();
    let on_keydown = move |e: web_sys::KeyboardEvent| match e.key().as_str() {
        "Enter" => {
            e.prevent_default();
            if e.shift_key() {
                go_prev();
            } else {
                go_next();
            }
        }
        "Escape" => {
            e.prevent_default();
            on_close_esc.run(());
        }
        _ => {}
    };

    let go_next_click = move |_: web_sys::MouseEvent| go_next();
    let go_prev_click = move |_: web_sys::MouseEvent| go_prev();
    let on_close_click = on_close.clone();

    view! {
        <div class="search-bar flex items-center gap-1 px-3 py-1.5 bg-bg-panel border-b border-border-subtle">
            // Search icon
            <svg class="w-3.5 h-3.5 text-text-muted shrink-0" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                <circle cx="7" cy="7" r="5" />
                <path d="M11 11l3 3" />
            </svg>
            // Input
            <input
                node_ref=input_ref
                class="search-bar-input flex-1 bg-transparent text-sm text-text outline-none placeholder-text-muted/50"
                prop:value=move || query.get()
                on:input=move |e| {
                    let v = event_target_value(&e);
                    set_query.set(v);
                }
                on:keydown=on_keydown
                placeholder="Search in conversation..."
            />
            // Regex toggle
            <button
                class=move || {
                    if use_regex.get() {
                        "search-bar-btn p-1 rounded text-primary bg-primary/10"
                    } else {
                        "search-bar-btn p-1 rounded text-text-muted hover:text-text"
                    }
                }
                on:click=move |_: web_sys::MouseEvent| set_use_regex.update(|v| *v = !*v)
                title="Toggle regex"
            >
                <svg class="w-3.5 h-3.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                    <text x="2" y="12" font-size="10" font-family="monospace" fill="currentColor" stroke="none">".*"</text>
                </svg>
            </button>
            // Match count
            {move || {
                let q = query.get();
                if q.is_empty() { return None; }
                let m = matches.get();
                let idx = active_index.get();
                Some(view! {
                    <span class="text-[10px] text-text-muted shrink-0 min-w-[60px] text-center">
                        {if m.is_empty() {
                            "No matches".to_string()
                        } else {
                            format!("{} of {}", idx + 1, m.len())
                        }}
                    </span>
                })
            }}
            // Nav buttons
            <button
                class="search-bar-btn p-1 rounded text-text-muted hover:text-text disabled:opacity-30"
                on:click=go_prev_click
                title="Previous match"
            >
                <svg class="w-3.5 h-3.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M4 10l4-4 4 4" />
                </svg>
            </button>
            <button
                class="search-bar-btn p-1 rounded text-text-muted hover:text-text disabled:opacity-30"
                on:click=go_next_click
                title="Next match"
            >
                <svg class="w-3.5 h-3.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M4 6l4 4 4-4" />
                </svg>
            </button>
            // Close
            <button
                class="search-bar-btn p-1 rounded text-text-muted hover:text-text"
                on:click=move |_: web_sys::MouseEvent| on_close_click.run(())
                title="Close search"
            >
                <svg class="w-3.5 h-3.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M4 4l8 8M12 4l-8 8" />
                </svg>
            </button>
        </div>
    }
}
