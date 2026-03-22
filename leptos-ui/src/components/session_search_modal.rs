//! SessionSearchModal — in-project fuzzy session search.
//! Matches React `SessionSearchModal.tsx`.

use crate::types::api::{ProjectInfo, SessionInfo};
use crate::utils::format_relative_time as format_timestamp;
use leptos::prelude::*;

// ── Fuzzy match ─────────────────────────────────────────────────────

#[derive(Clone)]
struct FuzzyResult {
    score: usize,
    indices: Vec<usize>,
}

fn fuzzy_match(query: &str, target: &str) -> Option<FuzzyResult> {
    if query.is_empty() {
        return None;
    }
    let query_lower = query.to_lowercase();
    let target_lower = target.to_lowercase();

    // Try substring match first
    if let Some(pos) = target_lower.find(&query_lower) {
        let indices: Vec<usize> = (pos..pos + query_lower.len()).collect();
        return Some(FuzzyResult {
            score: pos,
            indices,
        });
    }

    // Fuzzy character-by-character
    let target_chars: Vec<char> = target_lower.chars().collect();
    let query_chars: Vec<char> = query_lower.chars().collect();
    let mut indices = Vec::new();
    let mut ti = 0;
    let mut gaps = 0;

    for &qc in &query_chars {
        let mut found = false;
        while ti < target_chars.len() {
            if target_chars[ti] == qc {
                if !indices.is_empty() && ti > *indices.last().unwrap() + 1 {
                    gaps += 1;
                }
                indices.push(ti);
                ti += 1;
                found = true;
                break;
            }
            ti += 1;
        }
        if !found {
            return None;
        }
    }

    let first_index = indices.first().copied().unwrap_or(0);
    let len_diff = target_chars.len().abs_diff(query_chars.len());
    let score = 100 + gaps * 10 + first_index + len_diff;
    Some(FuzzyResult { score, indices })
}

const MAX_VISIBLE_RESULTS: usize = 30;

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn SessionSearchModal(
    project: ProjectInfo,
    project_idx: usize,
    on_select: Callback<(usize, String)>,
    on_close: Callback<()>,
) -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (selected, set_selected) = signal(0usize);
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let list_ref = NodeRef::<leptos::html::Div>::new();

    // Clone sessions from project
    let sessions = project.sessions.clone();

    // Parent sessions sorted by updated desc
    let parent_sessions = {
        let mut ps: Vec<SessionInfo> = sessions
            .into_iter()
            .filter(|s| s.parent_id.is_empty())
            .collect();
        ps.sort_by(|a, b| {
            b.time
                .updated
                .partial_cmp(&a.time.updated)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ps
    };

    let parent_sessions_stored = StoredValue::new(parent_sessions);

    // Computed results
    let results = Memo::new(move |_| {
        let q = query.get();
        let sessions = parent_sessions_stored.get_value();
        if q.is_empty() {
            return sessions
                .into_iter()
                .take(MAX_VISIBLE_RESULTS)
                .collect::<Vec<_>>();
        }
        let mut scored: Vec<(SessionInfo, usize)> = sessions
            .into_iter()
            .filter_map(|s| fuzzy_match(&q, &s.title).map(|r| (s, r.score)))
            .collect();
        scored.sort_by_key(|(_, score)| *score);
        scored
            .into_iter()
            .take(MAX_VISIBLE_RESULTS)
            .map(|(s, _)| s)
            .collect()
    });

    // Reset selected on results change
    Effect::new(move |_| {
        let _ = results.get();
        set_selected.set(0);
    });

    // Auto-focus input
    Effect::new(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    // Scroll selected into view
    Effect::new(move |_| {
        let idx = selected.get();
        if let Some(list) = list_ref.get() {
            let children = list.children();
            if let Some(child) = children.item(idx as u32) {
                let _ = child.scroll_into_view_with_bool(false);
            }
        }
    });

    let on_close_c = on_close.clone();
    let on_select_c = on_select.clone();

    let handle_keydown = move |e: leptos::ev::KeyboardEvent| {
        let key = e.key();
        match key.as_str() {
            "Escape" => {
                e.prevent_default();
                on_close_c.run(());
            }
            "ArrowDown" => {
                e.prevent_default();
                let len = results.get_untracked().len();
                if len > 0 {
                    set_selected.update(|s| *s = (*s + 1) % len);
                }
            }
            "ArrowUp" => {
                e.prevent_default();
                let len = results.get_untracked().len();
                if len > 0 {
                    set_selected.update(|s| {
                        if *s == 0 {
                            *s = len - 1;
                        } else {
                            *s -= 1;
                        }
                    });
                }
            }
            "Enter" => {
                e.prevent_default();
                let r = results.get_untracked();
                let idx = selected.get_untracked();
                if let Some(session) = r.get(idx) {
                    on_select_c.run((project_idx, session.id.clone()));
                }
            }
            _ => {}
        }
    };

    let on_backdrop = move |e: leptos::ev::MouseEvent| {
        use wasm_bindgen::JsCast;
        if let Some(target) = e.target() {
            if let Some(current) = e.current_target() {
                if target == current {
                    on_close.run(());
                }
            }
        }
    };

    let project_name = project.name.clone();
    let total_count = parent_sessions_stored.get_value().len();

    view! {
        <div
            class="search-modal-backdrop"
            on:click=on_backdrop
            on:keydown=handle_keydown
        >
            <div
                class="search-modal"
                role="dialog"
                aria-modal="true"
            >
                <div class="search-modal-header">
                    <span class="search-modal-title">"Search Sessions"</span>
                    <span class="search-modal-project">{project_name}</span>
                </div>
                <div class="search-modal-input-row">
                    <span class="search-modal-prompt">">"</span>
                    <input
                        class="search-modal-input"
                        type="text"
                        placeholder="Type to filter..."
                        spellcheck="false"
                        autocomplete="off"
                        node_ref=input_ref
                        prop:value=move || query.get()
                        on:input=move |e| {
                            use wasm_bindgen::JsCast;
                            let val = e.target()
                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                .map(|el| el.value())
                                .unwrap_or_default();
                            set_query.set(val);
                        }
                    />
                </div>
                <div class="search-modal-separator"/>
                <div class="search-modal-results" role="listbox" node_ref=list_ref>
                    {move || {
                        let r = results.get();
                        if r.is_empty() {
                            return view! {
                                <div class="search-modal-empty">"No matching sessions"</div>
                            }.into_any();
                        }
                        let q = query.get();
                        view! {
                            <div>
                                {r.into_iter().enumerate().map(|(i, session)| {
                                    let sid = session.id.clone();
                                    let sid_click = sid.clone();
                                    let title = session.title.clone();
                                    let ts = format_timestamp(session.time.updated);
                                    let pi = project_idx;
                                    let on_sel = on_select.clone();
                                    // Highlight matched chars
                                    let highlighted = if !q.is_empty() {
                                        if let Some(m) = fuzzy_match(&q, &title) {
                                            highlight_text(&title, &m.indices)
                                        } else {
                                            title.clone()
                                        }
                                    } else {
                                        title.clone()
                                    };

                                    view! {
                                        <div
                                            class=move || {
                                                if selected.get() == i {
                                                    "search-modal-result selected"
                                                } else {
                                                    "search-modal-result"
                                                }
                                            }
                                            role="option"
                                            aria-selected=move || (selected.get() == i).to_string()
                                            on:click=move |_| on_sel.run((pi, sid_click.clone()))
                                            on:mouseenter=move |_| set_selected.set(i)
                                        >
                                            <span class="search-result-title" inner_html=highlighted.clone()/>
                                            <span class="search-result-meta">{ts}</span>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </div>
                <div class="search-modal-footer">
                    <span class="search-modal-hint">
                        {"\u{2191}\u{2193} navigate  Enter select  Esc cancel"}
                    </span>
                    <span class="search-modal-count">
                        {move || format!("[{}/{}]", results.get().len(), total_count)}
                    </span>
                </div>
            </div>
        </div>
    }
}

fn highlight_text(text: &str, indices: &[usize]) -> String {
    let chars: Vec<char> = text.chars().collect();
    let index_set: std::collections::HashSet<usize> = indices.iter().copied().collect();
    let mut html = String::new();
    let mut in_mark = false;

    for (i, &ch) in chars.iter().enumerate() {
        let should_highlight = index_set.contains(&i);
        if should_highlight && !in_mark {
            html.push_str("<mark class=\"search-highlight\">");
            in_mark = true;
        } else if !should_highlight && in_mark {
            html.push_str("</mark>");
            in_mark = false;
        }
        // Escape HTML
        match ch {
            '<' => html.push_str("&lt;"),
            '>' => html.push_str("&gt;"),
            '&' => html.push_str("&amp;"),
            '"' => html.push_str("&quot;"),
            _ => html.push(ch),
        }
    }
    if in_mark {
        html.push_str("</mark>");
    }
    html
}
