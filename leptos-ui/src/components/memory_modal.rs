//! MemoryModal — CRUD for personal memory items with inline editing.
//! Matches React `memory-modal/MemoryModal.tsx`.

use leptos::prelude::*;
use serde::Serialize;
use crate::api::client::{api_delete, api_fetch, api_patch, api_post};
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{PersonalMemoryItem, PersonalMemoryListResponse, ProjectInfo};
use crate::components::icons::*;

// ── Helpers ─────────────────────────────────────────────────────────

const SCOPE_OPTIONS: &[&str] = &["global", "project", "session"];

fn format_scope(scope: &str) -> &'static str {
    match scope {
        "global" => "Global",
        "project" => "Project",
        "session" => "Session",
        _ => "Unknown",
    }
}

fn describe_scope(item: &PersonalMemoryItem, projects: &[ProjectInfo]) -> String {
    match item.scope.as_str() {
        "global" => "All work".to_string(),
        "project" => {
            item.project_index
                .and_then(|idx| projects.get(idx))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Project scope".to_string())
        }
        "session" => {
            item.session_id
                .as_ref()
                .map(|sid| format!("Session {}", &sid[..sid.len().min(8)]))
                .unwrap_or_else(|| "Session scope".to_string())
        }
        _ => "Unknown".to_string(),
    }
}

fn format_relative_date(iso: &str) -> String {
    // Simple fallback — just return the ISO date trimmed
    iso.chars().take(16).collect::<String>().replace('T', " ")
}

// ── API request bodies ──────────────────────────────────────────────

#[derive(Serialize)]
struct CreateMemoryBody {
    label: String,
    content: String,
    scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}

#[derive(Serialize)]
struct UpdateMemoryBody {
    label: String,
    content: String,
    scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn MemoryModal(
    on_close: Callback<()>,
    projects: Vec<ProjectInfo>,
    active_project_index: usize,
    active_session_id: Option<String>,
) -> impl IntoView {
    let (items, set_items) = signal(Vec::<PersonalMemoryItem>::new());
    let (loading, set_loading) = signal(true);
    let (saving, set_saving) = signal(false);

    // Create form
    let (label, set_label) = signal(String::new());
    let (content, set_content) = signal(String::new());
    let (scope, set_scope) = signal("global".to_string());

    // Inline edit state
    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (editing_label, set_editing_label) = signal(String::new());
    let (editing_content, set_editing_content) = signal(String::new());
    let (editing_scope, set_editing_scope) = signal("global".to_string());

    // Load on mount
    {
        leptos::task::spawn_local(async move {
            match api_fetch::<PersonalMemoryListResponse>("/memory").await {
                Ok(resp) => set_items.set(resp.memory),
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
                project_index: if s == "project" || s == "session" { Some(pi) } else { None },
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

    let active_session_id_ctx = active_session_id.clone();
    let context_text = move || {
        let s = scope.get();
        match s.as_str() {
            "global" => "Visible everywhere".to_string(),
            "project" => format!(
                "Applies to {}",
                projects_create
                    .get(active_project_index)
                    .map(|p| p.name.as_str())
                    .unwrap_or("current project")
            ),
            "session" => {
                active_session_id_ctx
                    .as_ref()
                    .map(|sid| format!("Applies to session {}", &sid[..sid.len().min(8)]))
                    .unwrap_or_else(|| "No active session to scope memory".to_string())
            }
            _ => String::new(),
        }
    };

    let projects_view = projects.clone();
    let active_session_id_edit = active_session_id;

    let handle_save_edit = Callback::new(move |_: web_sys::MouseEvent| {
        let eid = editing_id.get_untracked();
        let el = editing_label.get_untracked();
        let ec = editing_content.get_untracked();
        let es = editing_scope.get_untracked();
        if eid.is_none() || el.trim().is_empty() || ec.trim().is_empty() {
            return;
        }
        let eid = eid.unwrap();
        let pi = active_project_index;
        let sid = active_session_id_edit.clone();

        leptos::task::spawn_local(async move {
            let body = UpdateMemoryBody {
                label: el.trim().to_string(),
                content: ec.trim().to_string(),
                scope: es.clone(),
                project_index: if es == "project" || es == "session" { Some(pi) } else { None },
                session_id: if es == "session" { sid } else { None },
            };
            let path = format!("/memory/{}", eid);
            match api_patch::<PersonalMemoryItem>(&path, &body).await {
                Ok(updated) => {
                    set_items.update(|list| {
                        if let Some(entry) = list.iter_mut().find(|i| i.id == updated.id) {
                            *entry = updated;
                        }
                    });
                    set_editing_id.set(None);
                }
                Err(e) => leptos::logging::warn!("Failed to update memory: {}", e),
            }
        });
    });

    let cancel_edit = Callback::new(move |_: web_sys::MouseEvent| {
        set_editing_id.set(None);
    });

    view! {
        <ModalOverlay on_close=on_close class="memory-modal">
            <div class="memory-header">
                <div class="memory-header-left">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M12 2a8 8 0 0 0-8 8c0 6 8 12 8 12s8-6 8-12a8 8 0 0 0-8-8z" />
                    </svg>
                    <h3>"Personal Memory"</h3>
                    <span class="memory-count">{move || items.get().len()}</span>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close memory">
                    <IconX size=16 />
                </button>
            </div>

            <div class="memory-scrollable">
                // Create form
                <div class="memory-create">
                    <div class="memory-create-grid">
                        <input
                            class="memory-input"
                            prop:value=move || label.get()
                            on:input=move |ev| set_label.set(event_target_value(&ev))
                            placeholder="Memory label"
                        />
                        <select
                            class="memory-select"
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
                    <textarea
                        class="memory-textarea"
                        rows="3"
                        prop:value=move || content.get()
                        on:input=move |ev| set_content.set(event_target_value(&ev))
                        placeholder="Store a stable preference, recurring constraint, or working norm"
                    />
                    <div class="memory-create-footer">
                        <span class="memory-context">{context_text}</span>
                        <button
                            class="memory-create-btn"
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

                        // Group by scope
                        let projects_ref = &projects_view;
                        let sections = SCOPE_OPTIONS.iter().filter_map(|&s| {
                            let scoped: Vec<_> = all_items.iter().filter(|i| i.scope == s).cloned().collect();
                            if scoped.is_empty() {
                                return None;
                            }
                            let scope_label = format_scope(s);
                            let items_views = scoped.into_iter().map(|item| {
                                let item_id = item.id.clone();
                                let item_id_edit = item.id.clone();
                                let item_id_delete = item.id.clone();
                                let item_id_for_cancel = item.id.clone();
                                let item_id_for_edit_btn = item.id.clone();
                                let item_id_for_del_btn = item.id.clone();
                                let item_label_display = item.label.clone();
                                let item_content_display = item.content.clone();
                                let desc = describe_scope(&item, projects_ref);
                                let updated = format!("updated {}", format_relative_date(&item.updated_at));
                                let start_edit_id = item.id.clone();
                                let start_edit_label = item.label.clone();
                                let start_edit_content = item.content.clone();
                                let start_edit_scope = item.scope.clone();

                                view! {
                                    <div class="memory-item">
                                        <div class="memory-item-main">
                                            {move || {
                                                let eid = editing_id.get();
                                                if eid.as_ref() == Some(&item_id_edit) {
                                                    view! {
                                                        <div>
                                                            <input
                                                                class="memory-input"
                                                                prop:value=move || editing_label.get()
                                                                on:input=move |ev| set_editing_label.set(event_target_value(&ev))
                                                            />
                                                            <select
                                                                class="memory-select"
                                                                prop:value=move || editing_scope.get()
                                                                on:change=move |ev| set_editing_scope.set(event_target_value(&ev))
                                                            >
                                                                {SCOPE_OPTIONS.iter().map(|opt| {
                                                                    let val = opt.to_string();
                                                                    let display = format_scope(opt);
                                                                    view! { <option value=val>{display}</option> }
                                                                }).collect::<Vec<_>>()}
                                                            </select>
                                                            <textarea
                                                                class="memory-textarea"
                                                                rows="3"
                                                                prop:value=move || editing_content.get()
                                                                on:input=move |ev| set_editing_content.set(event_target_value(&ev))
                                                            />
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    view! {
                                                        <div>
                                                            <div class="memory-item-label">{item_label_display.clone()}</div>
                                                            <div class="memory-item-content">{item_content_display.clone()}</div>
                                                        </div>
                                                    }.into_any()
                                                }
                                            }}
                                            <div class="memory-item-meta">
                                                <span>{desc.clone()}</span>
                                                <span>{updated.clone()}</span>
                                            </div>
                                        </div>
                                        <div class="memory-item-actions">
                                            // Save / Cancel (editing mode)
                                            <button
                                                class="memory-edit-btn"
                                                style=move || if editing_id.get().as_ref() == Some(&item_id) { "" } else { "display:none" }
                                                on:click=move |ev| handle_save_edit.run(ev)
                                                aria-label="Save"
                                            >
                                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                    <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
                                                    <polyline points="17 21 17 13 7 13 7 21" />
                                                    <polyline points="7 3 7 8 15 8" />
                                                </svg>
                                            </button>
                                            <button
                                                class="memory-delete-btn"
                                                style=move || if editing_id.get().as_ref() == Some(&item_id_for_cancel) { "" } else { "display:none" }
                                                on:click=move |ev| cancel_edit.run(ev)
                                                aria-label="Cancel"
                                            >
                                                <IconX size=14 />
                                            </button>
                                            // Edit / Delete (view mode)
                                            <button
                                                class="memory-edit-btn"
                                                style=move || if editing_id.get().as_ref() != Some(&item_id_for_edit_btn) { "" } else { "display:none" }
                                                on:click={
                                                    let eid = start_edit_id.clone();
                                                    let el = start_edit_label.clone();
                                                    let ec = start_edit_content.clone();
                                                    let es = start_edit_scope.clone();
                                                    move |_: web_sys::MouseEvent| {
                                                        set_editing_id.set(Some(eid.clone()));
                                                        set_editing_label.set(el.clone());
                                                        set_editing_content.set(ec.clone());
                                                        set_editing_scope.set(es.clone());
                                                    }
                                                }
                                                aria-label="Edit"
                                            >
                                                <IconPenSquare size=14 />
                                            </button>
                                            <button
                                                class="memory-delete-btn"
                                                style=move || if editing_id.get().as_ref() != Some(&item_id_for_del_btn) { "" } else { "display:none" }
                                                on:click=move |_: web_sys::MouseEvent| {
                                                    let del_id = item_id_delete.clone();
                                                    leptos::task::spawn_local(async move {
                                                        let path = format!("/memory/{}", del_id);
                                                        if api_delete(&path).await.is_ok() {
                                                            set_items.update(|list| list.retain(|i| i.id != del_id));
                                                        }
                                                    });
                                                }
                                                aria-label="Delete"
                                            >
                                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                    <polyline points="3 6 5 6 21 6" />
                                                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                                                </svg>
                                            </button>
                                        </div>
                                    </div>
                                }
                            }).collect::<Vec<_>>();

                            Some(view! {
                                <section class="memory-section">
                                    <div class="memory-section-title">{scope_label}</div>
                                    <div>{items_views}</div>
                                </section>
                            })
                        }).collect_view();

                        view! { <div>{sections}</div> }.into_any()
                    }}
                </div>
            </div>
        </ModalOverlay>
    }
}
