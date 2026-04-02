//! MemoryModal — CRUD for personal memory items with inline editing.
//! Split: main list + keyboard nav (this file), item row (item_view.rs),
//! helpers + API bodies (helpers.rs).

mod helpers;
mod item_view;

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use crate::api::client::{api_fetch, api_post};
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{PersonalMemoryItem, PersonalMemoryListResponse, ProjectInfo};
use crate::components::icons::*;
use helpers::{format_scope, CreateMemoryBody, SCOPE_OPTIONS};
use item_view::MemoryItemRow;

#[component]
pub fn MemoryModal(
    on_close: Callback<()>,
    projects: Vec<ProjectInfo>,
    active_project_index: usize,
    active_session_id: Option<String>,
    #[prop(optional)] filter_active_only: Option<ReadSignal<bool>>,
) -> impl IntoView {
    let show_active_only = filter_active_only
        .map(|s| s.get_untracked())
        .unwrap_or(false);

    let (items, set_items) = signal(Vec::<PersonalMemoryItem>::new());
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);
    let (selected_idx, set_selected_idx) = signal(0usize);

    // Create form
    let (label, set_label) = signal(String::new());
    let (content, set_content) = signal(String::new());
    let (scope, set_scope) = signal("global".to_string());

    // Inline edit state
    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (editing_label, set_editing_label) = signal(String::new());
    let (editing_content, set_editing_content) = signal(String::new());
    let (editing_scope, set_editing_scope) = signal("global".to_string());

    // Load on mount — active-only or all memories
    {
        let pi = active_project_index;
        let sid = active_session_id.clone();
        leptos::task::spawn_local(async move {
            match api_fetch::<PersonalMemoryListResponse>("/memory").await {
                Ok(resp) => {
                    if show_active_only {
                        // Filter to memories applicable to this session context:
                        // global → always, project → matching project_index, session → matching session_id
                        let filtered = resp.memory.into_iter().filter(|item| {
                            match item.scope.as_str() {
                                "global" => true,
                                "project" => item.project_index == Some(pi),
                                "session" => {
                                    sid.as_ref().map_or(false, |s| {
                                        item.session_id.as_ref() == Some(s)
                                    })
                                }
                                _ => false,
                            }
                        }).collect();
                        set_items.set(filtered);
                    } else {
                        set_items.set(resp.memory);
                    }
                }
                Err(e) => leptos::logging::warn!("Failed to load memory: {}", e),
            }
            set_loading.set(false);
        });
    }

    let projects_create = projects.clone();
    let active_session_id_create = active_session_id.clone();
    let handle_create = move |_: web_sys::MouseEvent| {
        let l = label.get_untracked();
        let c = content.get_untracked();
        if l.trim().is_empty() || c.trim().is_empty() {
            return;
        }
        let s = scope.get_untracked();
        let pi = active_project_index;
        let sid = active_session_id_create.clone();

        set_saving.set(true);
        leptos::task::spawn_local(async move {
            let body = CreateMemoryBody {
                label: l.trim().to_string(),
                content: c.trim().to_string(),
                scope: s.clone(),
                project_index: if s == "project" || s == "session" {
                    Some(pi)
                } else {
                    None
                },
                session_id: if s == "session" { sid } else { None },
            };
            match api_post::<PersonalMemoryItem>("/memory", &body).await {
                Ok(created) => {
                    set_items.update(|list| list.insert(0, created));
                    set_label.set(String::new());
                    set_content.set(String::new());
                    set_scope.set("global".to_string());
                }
                Err(e) => leptos::logging::warn!("Failed to create memory: {}", e),
            }
            set_saving.set(false);
        });
    };

    let context_text = {
        let projects_ctx = projects_create.clone();
        let sid_ctx = active_session_id.clone();
        move || {
            let s = scope.get();
            match s.as_str() {
                "global" => "Visible everywhere".to_string(),
                "project" => format!(
                    "Applies to {}",
                    projects_ctx
                        .get(active_project_index)
                        .map(|p| p.name.as_str())
                        .unwrap_or("current project")
                ),
                "session" => {
                    sid_ctx
                        .as_ref()
                        .map(|sid| format!("Applies to session {}", &sid[..sid.len().min(8)]))
                        .unwrap_or_else(|| "No active session".to_string())
                }
                _ => String::new(),
            }
        }
    };

    // Scroll selected into view
    Effect::new(move |_| {
        let idx = selected_idx.get();
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            let sel = format!("[data-mem-idx=\"{}\"]", idx);
            if let Ok(Some(el)) = doc.query_selector(&sel) {
                if let Some(html_el) = el.dyn_ref::<web_sys::HtmlElement>() {
                    html_el.scroll_into_view();
                }
            }
        }
    });

    let on_keydown = {
        let items_ref = items;
        move |e: web_sys::KeyboardEvent| {
            match e.key().as_str() {
                "ArrowDown" | "j" => {
                    e.prevent_default();
                    let len = items_ref.get_untracked().len();
                    if len > 0 {
                        set_selected_idx.update(|i| *i = (*i + 1) % len);
                    }
                }
                "ArrowUp" | "k" => {
                    e.prevent_default();
                    let len = items_ref.get_untracked().len();
                    if len > 0 {
                        set_selected_idx.update(|i| {
                            *i = if *i == 0 { len - 1 } else { *i - 1 };
                        });
                    }
                }
                "Enter" => {
                    // Toggle inline edit for selected item
                    let all = items_ref.get_untracked();
                    let idx = selected_idx.get_untracked();
                    if let Some(item) = all.get(idx) {
                        if editing_id.get_untracked().as_ref() == Some(&item.id) {
                            return; // already editing — let form handle Enter
                        }
                        set_editing_id.set(Some(item.id.clone()));
                        set_editing_label.set(item.label.clone());
                        set_editing_content.set(item.content.clone());
                        set_editing_scope.set(item.scope.clone());
                    }
                }
                _ => {}
            }
        }
    };

    let projects_view = projects.clone();

    view! {
        <ModalOverlay on_close=on_close class="memory-modal">
            <div class="memory-header">
                <div class="memory-header-left">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                        stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M12 2a8 8 0 0 0-8 8c0 6 8 12 8 12s8-6 8-12a8 8 0 0 0-8-8z" />
                    </svg>
                    <h3>{if show_active_only { "Active Memory" } else { "Personal Memory" }}</h3>
                    <span class="memory-count">{move || items.get().len()}</span>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close memory">
                    <IconX size=16 />
                </button>
            </div>

            <div class="memory-scrollable" on:keydown=on_keydown tabindex=0>
                // Create form
                <div class="memory-create">
                    <div class="memory-create-grid">
                        <input class="memory-input"
                            prop:value=move || label.get()
                            on:input=move |ev| set_label.set(event_target_value(&ev))
                            placeholder="Memory label" />
                        <select class="memory-select"
                            prop:value=move || scope.get()
                            on:change=move |ev| set_scope.set(event_target_value(&ev))
                        >
                            {SCOPE_OPTIONS.iter().map(|opt| {
                                let val = opt.to_string();
                                let display = format_scope(opt);
                                view! { <option value=val>{display}</option> }
                            }).collect::<Vec<_>>()}
                        </select>
                    </div>
                    <textarea class="memory-textarea" rows="3"
                        prop:value=move || content.get()
                        on:input=move |ev| set_content.set(event_target_value(&ev))
                        placeholder="Store a stable preference, recurring constraint, or working norm" />
                    <div class="memory-create-footer">
                        <span class="memory-context">{context_text}</span>
                        <button class="memory-create-btn"
                            on:click=handle_create
                            disabled=move || saving.get() || label.get().trim().is_empty() || content.get().trim().is_empty()
                        >
                            <IconPlus size=14 />
                            {move || if saving.get() { " Saving..." } else { " Save memory" }}
                        </button>
                    </div>
                </div>

                // Body
                <div class="memory-body">
                    {move || {
                        if loading.get() {
                            return view! { <div class="memory-empty">"Loading memory..."</div> }.into_any();
                        }
                        let all_items = items.get();
                        if all_items.is_empty() {
                            return view! { <div class="memory-empty">"No personal memory yet."</div> }.into_any();
                        }
                        let sel = selected_idx.get();
                        let projects_ref = projects_view.clone();
                        let sid_ref = active_session_id.clone();

                        let sections = SCOPE_OPTIONS.iter().filter_map(|&s| {
                            let scoped: Vec<_> = all_items.iter()
                                .enumerate()
                                .filter(|(_, i)| i.scope == s)
                                .collect();
                            if scoped.is_empty() { return None; }
                            let scope_label = helpers::format_scope(s);
                            let rows = scoped.into_iter().map(|(idx, item)| {
                                view! {
                                    <MemoryItemRow
                                        item=item.clone() is_selected={idx == sel} idx=idx
                                        projects=projects_ref.clone()
                                        active_project_index=active_project_index
                                        active_session_id=sid_ref.clone()
                                        set_items=set_items
                                        editing_id=editing_id set_editing_id=set_editing_id
                                        editing_label=editing_label set_editing_label=set_editing_label
                                        editing_content=editing_content set_editing_content=set_editing_content
                                        editing_scope=editing_scope set_editing_scope=set_editing_scope
                                        on_hover=Callback::new(move |i| set_selected_idx.set(i))
                                    />
                                }
                            }).collect::<Vec<_>>();
                            Some(view! {
                                <section class="memory-section">
                                    <div class="memory-section-title">{scope_label}</div>
                                    <div>{rows}</div>
                                </section>
                            })
                        }).collect_view();

                        view! { <div>{sections}</div> }.into_any()
                    }}
                </div>
            </div>

            <div class="memory-footer">
                <kbd>"Up/Down"</kbd>" Navigate "<kbd>"Enter"</kbd>" Edit "<kbd>"Esc"</kbd>" Close"
            </div>
        </ModalOverlay>
    }
}
