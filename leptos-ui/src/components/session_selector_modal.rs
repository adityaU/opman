//! SessionSelectorModal — browse & switch sessions across all projects.
//! Matches React `SessionSelectorModal.tsx`.

use crate::components::icons::*;
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{ProjectInfo, SessionInfo};
use leptos::prelude::*;

// ── Types ───────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct FlatSession {
    project_name: String,
    project_idx: usize,
    session: SessionInfo,
    is_current: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────

fn relative_time(ts: f64) -> String {
    let now = js_sys::Date::now() / 1000.0;
    let diff = (now - ts).max(0.0) as u64;
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

// ── Component ───────────────────────────────────────────────────────

/// Session selector modal component.
#[component]
pub fn SessionSelectorModal(
    on_close: Callback<()>,
    projects: Vec<ProjectInfo>,
    active_session_id: Option<String>,
    on_select_session: Callback<(String, usize)>,
) -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (selected_idx, set_selected_idx) = signal(0usize);
    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Focus input
    Effect::new(move |_| {
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    // Flatten all sessions
    let all_sessions: Vec<FlatSession> = {
        let active_id = active_session_id.clone();
        let mut flat: Vec<FlatSession> = projects
            .iter()
            .flat_map(|p| {
                let active_ref = active_id.as_deref();
                let pname = p.name.clone();
                let pidx = p.index;
                p.sessions.iter().map(move |s| FlatSession {
                    project_name: pname.clone(),
                    project_idx: pidx,
                    is_current: active_ref == Some(s.id.as_str()),
                    session: s.clone(),
                })
            })
            .collect();
        // Sort by updated time, most recent first
        flat.sort_by(|a, b| {
            b.session
                .time
                .updated
                .partial_cmp(&a.session.time.updated)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        flat
    };

    // Filtered sessions memo
    let all_clone = all_sessions.clone();
    let filtered = Memo::new(move |_| {
        let q = query.get().to_lowercase();
        if q.is_empty() {
            return all_clone.clone();
        }
        all_clone
            .iter()
            .filter(|s| {
                s.project_name.to_lowercase().contains(&q)
                    || s.session.title.to_lowercase().contains(&q)
                    || s.session.id.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    });

    // Reset on query change
    Effect::new(move |_| {
        let _ = query.get();
        set_selected_idx.set(0);
    });

    let on_close2 = on_close;
    let on_select = on_select_session;

    let on_keydown = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        match key.as_str() {
            "ArrowDown" => {
                e.prevent_default();
                let len = filtered.get_untracked().len();
                if len > 0 {
                    set_selected_idx.update(|i| *i = (*i + 1) % len);
                }
            }
            "ArrowUp" => {
                e.prevent_default();
                let len = filtered.get_untracked().len();
                if len > 0 {
                    set_selected_idx.update(|i| {
                        *i = if *i == 0 { len - 1 } else { *i - 1 };
                    });
                }
            }
            "Enter" => {
                e.prevent_default();
                let items = filtered.get_untracked();
                let idx = selected_idx.get_untracked();
                if let Some(entry) = items.get(idx) {
                    on_select.run((entry.session.id.clone(), entry.project_idx));
                    on_close2.run(());
                }
            }
            _ => {}
        }
    };

    view! {
        <ModalOverlay on_close=on_close class="session-selector-modal">
            <div class="session-selector-header">
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <rect width="7" height="7" x="14" y="3" rx="1"/><rect width="7" height="7" x="14" y="14" rx="1"/><rect width="7" height="7" x="3" y="14" rx="1"/><rect width="7" height="7" x="3" y="3" rx="1"/>
                </svg>
                <span>"Select Session"</span>
                <span class="session-selector-count">
                    {move || format!("{} session(s)", filtered.get().len())}
                </span>
                <button class="session-selector-close" on:click=move |_| on_close.run(())>
                    <IconX size=14 class="w-3.5 h-3.5" />
                </button>
            </div>
            <div class="session-selector-search">
                <IconSearch size=14 class="w-3.5 h-3.5" />
                <input
                    class="session-selector-input"
                    node_ref=input_ref
                    type="text"
                    placeholder="Search sessions..."
                    prop:value=move || query.get()
                    on:input=move |e| set_query.set(event_target_value(&e))
                    on:keydown=on_keydown
                />
            </div>
            <div class="session-selector-results">
                {move || {
                    let items = filtered.get();
                    let sel = selected_idx.get();
                    if items.is_empty() {
                        view! { <div class="session-selector-empty">"No sessions found"</div> }.into_any()
                    } else {
                        items.into_iter().enumerate().map(|(idx, entry)| {
                            let is_selected = idx == sel;
                            let is_current = entry.is_current;
                            let class_str = format!(
                                "session-selector-item{}{}",
                                if is_selected { " selected" } else { "" },
                                if is_current { " current" } else { "" }
                            );
                            let sid = entry.session.id.clone();
                            let pidx = entry.project_idx;
                            let title = if entry.session.title.is_empty() {
                                entry.session.id[..8.min(entry.session.id.len())].to_string()
                            } else {
                                entry.session.title.clone()
                            };
                            let time_str = relative_time(entry.session.time.updated);
                            let project_name = entry.project_name.clone();

                            view! {
                                <button
                                    class=class_str
                                    on:click=move |_| {
                                        on_select.run((sid.clone(), pidx));
                                        on_close.run(());
                                    }
                                    on:mouseenter=move |_| set_selected_idx.set(idx)
                                >
                                    <div class="session-selector-item-left">
                                        <span class="session-selector-project">{project_name}</span>
                                        <span class="session-selector-sep">"/"</span>
                                        <span class="session-selector-title">{title}</span>
                                        {if is_current {
                                            Some(view! { <span class="session-selector-badge">"current"</span> })
                                        } else {
                                            None
                                        }}
                                    </div>
                                    <span class="session-selector-time">{time_str}</span>
                                </button>
                            }
                        }).collect_view().into_any()
                    }
                }}
            </div>
            <div class="session-selector-footer">
                <kbd>"Up/Down"</kbd>" Navigate "
                <kbd>"Enter"</kbd>" Select "
                <kbd>"Esc"</kbd>" Close"
            </div>
        </ModalOverlay>
    }
}
