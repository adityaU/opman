//! CrossSessionSearchModal — search across all sessions in a project.
//! Matches React `CrossSessionSearchModal.tsx`.

use crate::api::client::api_fetch;
use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{SearchResponse, SearchResultEntry};
use leptos::prelude::*;

fn format_timestamp(ts: f64) -> String {
    if ts == 0.0 {
        return String::new();
    }
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ts * 1000.0));
    let hours = d.get_hours();
    let mins = d.get_minutes();
    format!("{:02}:{:02}", hours, mins)
}

struct SessionGroup {
    session_id: String,
    session_title: String,
    entries: Vec<SearchResultEntry>,
}

fn group_by_session(results: &[SearchResultEntry]) -> Vec<SessionGroup> {
    let mut groups: Vec<SessionGroup> = Vec::new();
    for entry in results {
        if let Some(g) = groups.iter_mut().find(|g| g.session_id == entry.session_id) {
            g.entries.push(entry.clone());
        } else {
            let title = if entry.session_title.is_empty() {
                entry.session_id.chars().take(8).collect()
            } else {
                entry.session_title.clone()
            };
            groups.push(SessionGroup {
                session_id: entry.session_id.clone(),
                session_title: title,
                entries: vec![entry.clone()],
            });
        }
    }
    groups
}

/// CrossSessionSearchModal component.
#[component]
pub fn CrossSessionSearchModal(
    on_close: Callback<()>,
    project_idx: usize,
    on_navigate: Callback<String>,
) -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (results, set_results) = signal(Vec::<SearchResultEntry>::new());
    let (total, set_total) = signal(0usize);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (selected_index, set_selected_index) = signal(0usize);
    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Focus on mount
    Effect::new(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    // Debounced search
    Effect::new(move |_| {
        let q = query.get();
        let q_trimmed = q.trim().to_string();

        if q_trimmed.is_empty() {
            set_results.set(vec![]);
            set_total.set(0);
            set_error.set(None);
            return;
        }

        set_loading.set(true);
        set_error.set(None);

        let pidx = project_idx;
        leptos::task::spawn_local(async move {
            // Small delay for debounce effect
            gloo_timers::future::TimeoutFuture::new(300).await;

            let path = format!(
                "/project/{}/search?q={}&limit=50",
                pidx,
                js_sys::encode_uri_component(&q_trimmed)
            );
            match api_fetch::<SearchResponse>(&path).await {
                Ok(resp) => {
                    set_results.set(resp.results);
                    set_total.set(resp.total);
                    set_selected_index.set(0);
                }
                Err(e) => {
                    set_error.set(Some(format!("Search failed: {}", e)));
                    set_results.set(vec![]);
                    set_total.set(0);
                }
            }
            set_loading.set(false);
        });
    });

    let handle_navigate = move |session_id: String| {
        on_navigate.run(session_id);
        on_close.run(());
    };

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        match key.as_str() {
            "ArrowDown" => {
                e.prevent_default();
                let len = results.get_untracked().len();
                if len > 0 {
                    set_selected_index.update(|i| *i = (*i + 1).min(len - 1));
                }
            }
            "ArrowUp" => {
                e.prevent_default();
                set_selected_index.update(|i| *i = i.saturating_sub(1));
            }
            "Enter" => {
                e.prevent_default();
                let items = results.get_untracked();
                let idx = selected_index.get_untracked();
                if let Some(entry) = items.get(idx) {
                    handle_navigate(entry.session_id.clone());
                }
            }
            _ => {}
        }
    };

    view! {
        <ModalOverlay on_close=on_close class="cross-session-search-modal">
            <div class="cross-session-search-header">
                <IconSearch size=16 class="w-4 h-4 cross-session-search-icon" />
                <input
                    class="cross-session-search-input"
                    node_ref=input_ref
                    type="text"
                    placeholder="Search across all sessions..."
                    prop:value=move || query.get()
                    on:input=move |e| set_query.set(event_target_value(&e))
                    on:keydown=on_keydown
                />
                {move || if loading.get() {
                    Some(view! {
                        <IconLoader2 size=14 class="w-3.5 h-3.5 spinning cross-session-search-spinner" />
                    })
                } else { None }}
            </div>

            <div class="cross-session-search-body">
                {move || if let Some(err) = error.get() {
                    Some(view! { <div class="cross-session-search-error">{err}</div> }.into_any())
                } else { None }}

                {move || {
                    let q = query.get();
                    if q.trim().is_empty() {
                        Some(view! {
                            <div class="cross-session-search-hint">
                                "Type to search message content, tool calls, and outputs across all sessions in this project."
                            </div>
                        }.into_any())
                    } else { None }
                }}

                {move || {
                    let q = query.get();
                    let r = results.get();
                    if !q.trim().is_empty() && !loading.get() && r.is_empty() && error.get().is_none() {
                        Some(view! {
                            <div class="cross-session-search-empty">
                                {format!("No results found for \"{}\"", q)}
                            </div>
                        }.into_any())
                    } else { None }
                }}

                {move || {
                    let r = results.get();
                    if r.is_empty() { return None; }
                    let groups = group_by_session(&r);
                    let selected = selected_index.get();

                    // Build flat index mapping
                    let mut flat_idx = 0usize;

                    Some(view! {
                        <div>
                            {groups.into_iter().map(|group| {
                                let title = group.session_title.clone();
                                let match_count = group.entries.len();
                                let entries_with_idx: Vec<(usize, SearchResultEntry)> = group.entries.into_iter().map(|e| {
                                    let idx = flat_idx;
                                    flat_idx += 1;
                                    (idx, e)
                                }).collect();
                                view! {
                                    <div class="cross-session-search-group">
                                        <div class="cross-session-search-group-header">
                                            <span class="cross-session-search-session-title">{title}</span>
                                            <span class="cross-session-search-match-count">
                                                {format!("{} match{}", match_count, if match_count != 1 { "es" } else { "" })}
                                            </span>
                                        </div>
                                        {entries_with_idx.into_iter().map(|(global_idx, entry)| {
                                            let is_selected = global_idx == selected;
                                            let sid = entry.session_id.clone();
                                            let role_icon = if entry.role == "user" { "U" } else { "B" };
                                            let time_str = format_timestamp(entry.timestamp);
                                            let snippet = entry.snippet.clone();
                                            let cls = format!("cross-session-search-result{}", if is_selected { " selected" } else { "" });
                                            let role = entry.role.clone();
                                            view! {
                                                <button class=cls
                                                    on:click=move |_| handle_navigate(sid.clone())
                                                    on:mouseenter=move |_| set_selected_index.set(global_idx)>
                                                    <div class="cross-session-search-result-meta">
                                                        <span class="cross-session-search-role-icon">{role_icon}</span>
                                                        <span class="cross-session-search-role">{role}</span>
                                                        <span class="cross-session-search-time">{time_str}</span>
                                                    </div>
                                                    <div class="cross-session-search-snippet">{snippet}</div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    })
                }}

                {move || {
                    let t = total.get();
                    let r = results.get().len();
                    if t > r {
                        Some(view! {
                            <div class="cross-session-search-more">
                                {format!("Showing {} of {} results", r, t)}
                            </div>
                        })
                    } else { None }
                }}
            </div>
        </ModalOverlay>
    }
}
