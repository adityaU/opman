//! DelegationBoardModal — CRUD for delegated work items.
//! Matches React `DelegationBoardModal.tsx`.

use leptos::prelude::*;
use serde::Serialize;
use crate::api::client::{api_delete, api_fetch, api_patch, api_post};
use crate::components::modal_overlay::ModalOverlay;
use crate::types::api::{DelegatedWorkItem, DelegatedWorkListResponse, Mission};
use crate::components::icons::*;

#[derive(Serialize)]
struct CreateDelegatedWorkBody {
    title: String,
    assignee: String,
    scope: String,
    mission_id: Option<String>,
    session_id: Option<String>,
    subagent_session_id: Option<String>,
}

#[derive(Serialize)]
struct UpdateStatusBody {
    status: String,
}

#[component]
pub fn DelegationBoardModal(
    on_close: Callback<()>,
    missions: Vec<Mission>,
    active_session_id: Option<String>,
    #[prop(optional)]
    on_open_session: Option<Callback<String>>,
) -> impl IntoView {
    let (items, set_items) = signal(Vec::<DelegatedWorkItem>::new());
    let (loading, set_loading) = signal(true);
    let (title, set_title) = signal(String::new());
    let (assignee, set_assignee) = signal("build".to_string());
    let (scope, set_scope) = signal(String::new());
    let (mission_id, set_mission_id) = signal(String::new());
    let (subagent_session_id, set_subagent_session_id) = signal(String::new());

    // Load delegation items on mount
    {
        let set_items = set_items;
        let set_loading = set_loading;
        leptos::task::spawn_local(async move {
            match api_fetch::<DelegatedWorkListResponse>("/delegation").await {
                Ok(resp) => {
                    set_items.set(resp.items);
                }
                Err(e) => {
                    leptos::logging::warn!("Failed to load delegation board: {}", e);
                }
            }
            set_loading.set(false);
        });
    }

    let active_session_id_create = active_session_id.clone();
    let handle_create = move |_: web_sys::MouseEvent| {
        let t = title.get_untracked();
        let s = scope.get_untracked();
        if t.trim().is_empty() || s.trim().is_empty() {
            return;
        }
        let a = assignee.get_untracked();
        let m = mission_id.get_untracked();
        let sub = subagent_session_id.get_untracked();
        let sid = active_session_id_create.clone();
        let set_items = set_items;

        leptos::task::spawn_local(async move {
            let body = CreateDelegatedWorkBody {
                title: t.trim().to_string(),
                assignee: a.trim().to_string(),
                scope: s.trim().to_string(),
                mission_id: if m.is_empty() { None } else { Some(m) },
                session_id: sid,
                subagent_session_id: if sub.is_empty() { None } else { Some(sub) },
            };
            match api_post::<DelegatedWorkItem>("/delegation", &body).await {
                Ok(item) => {
                    set_items.update(|list| list.insert(0, item));
                    set_title.set(String::new());
                    set_scope.set(String::new());
                    set_mission_id.set(String::new());
                    set_subagent_session_id.set(String::new());
                }
                Err(e) => {
                    leptos::logging::warn!("Failed to create delegation: {}", e);
                }
            }
        });
    };

    let active_session_display = active_session_id
        .as_ref()
        .map(|s| s.chars().take(8).collect::<String>())
        .unwrap_or_else(|| "none".to_string());

    let statuses: Vec<&'static str> = vec!["planned", "running", "completed"];

    view! {
        <ModalOverlay on_close=on_close class="delegation-modal">
            <div class="delegation-header">
                <div class="delegation-header-left">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="2" y="7" width="20" height="14" rx="2" ry="2" />
                        <path d="M16 21V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16" />
                    </svg>
                    <h3>"Delegation Board"</h3>
                </div>
                <button on:click=move |_| on_close.run(()) aria-label="Close delegation board">
                    <IconX size=16 />
                </button>
            </div>

            <div class="delegation-scrollable">
                // Create form
                <div class="delegation-create">
                    <input
                        class="delegation-input"
                        prop:value=move || title.get()
                        on:input=move |ev| set_title.set(event_target_value(&ev))
                        placeholder="Delegated task title"
                    />
                    <input
                        class="delegation-input"
                        prop:value=move || assignee.get()
                        on:input=move |ev| set_assignee.set(event_target_value(&ev))
                        placeholder="Assignee"
                    />
                    <textarea
                        class="delegation-textarea"
                        prop:value=move || scope.get()
                        on:input=move |ev| set_scope.set(event_target_value(&ev))
                        rows="3"
                        placeholder="Bounded task scope"
                    />
                    <div class="delegation-create-grid">
                        <select
                            class="delegation-select"
                            prop:value=move || mission_id.get()
                            on:change=move |ev| set_mission_id.set(event_target_value(&ev))
                        >
                            <option value="">"No mission link"</option>
                            {missions.iter().map(|m| {
                                let id = m.id.clone();
                                let goal = m.goal.clone();
                                view! { <option value=id>{goal}</option> }
                            }).collect::<Vec<_>>()}
                        </select>
                        <input
                            class="delegation-input"
                            prop:value=move || subagent_session_id.get()
                            on:input=move |ev| set_subagent_session_id.set(event_target_value(&ev))
                            placeholder="Subagent session ID (optional)"
                        />
                    </div>
                    <div class="delegation-create-footer">
                        <span class="delegation-context">
                            "Current session: " {active_session_display}
                        </span>
                        <button
                            class="delegation-create-btn"
                            on:click=handle_create
                            disabled=move || title.get().trim().is_empty() || scope.get().trim().is_empty()
                        >
                            <IconPlus size=14 />
                            " Add delegated work"
                        </button>
                    </div>
                </div>

                // Item list
                <div class="delegation-body">
                    {move || {
                        if loading.get() {
                            view! { <div class="delegation-empty">"Loading delegation board..."</div> }.into_any()
                        } else if items.get().is_empty() {
                            view! { <div class="delegation-empty">"No delegated work yet."</div> }.into_any()
                        } else {
                            let item_list = items.get();
                            view! {
                                <div>
                                    {item_list.into_iter().map(|item| {
                                        let item_id = item.id.clone();
                                        let item_id_delete = item.id.clone();
                                        let title = item.title.clone();
                                        let scope_text = item.scope.clone();
                                        let assignee_text = item.assignee.clone();
                                        let status = item.status.clone();
                                        let subagent_sid = item.subagent_session_id.clone();
                                        let session_sid = item.session_id.clone();

                                        // Delete handler
                                        let handle_delete = {
                                            let set_items = set_items;
                                            let del_id = item_id_delete.clone();
                                            move |_: web_sys::MouseEvent| {
                                                let del_id = del_id.clone();
                                                leptos::task::spawn_local(async move {
                                                    let path = format!("/delegation/{}", del_id);
                                                    if api_delete(&path).await.is_ok() {
                                                        set_items.update(|list| list.retain(|i| i.id != del_id));
                                                    }
                                                });
                                            }
                                        };

                                        // Open session handlers
                                        let on_open = on_open_session;
                                        let subagent_sid_btn = subagent_sid.clone();
                                        let session_sid_btn = session_sid.clone();
                                        let has_subagent = subagent_sid.is_some();
                                        let has_session = session_sid.is_some();

                                        let statuses_clone = statuses.clone();
                                        let status_clone = status.clone();

                                        view! {
                                            <div class="delegation-item">
                                                <div class="delegation-item-main">
                                                    <div class="delegation-item-title">{title}</div>
                                                    <div class="delegation-item-scope">{scope_text}</div>
                                                    <div class="delegation-item-meta">
                                                        <span>{assignee_text}</span>
                                                        <span>{status_clone}</span>
                                                        {subagent_sid.map(|sid| view! { <span>"subagent " {sid}</span> })}
                                                    </div>
                                                    <div class="delegation-status-actions">
                                                        {statuses_clone.into_iter().map(|s| {
                                                            let active = status == s;
                                                            let id = item_id.clone();
                                                            let status_str = s.to_string();
                                                            view! {
                                                                <button
                                                                    class=if active { "delegation-status-btn active" } else { "delegation-status-btn" }
                                                                    on:click=move |_| {
                                                                        let id = id.clone();
                                                                        let status_str = status_str.clone();
                                                                        let set_items = set_items;
                                                                        leptos::task::spawn_local(async move {
                                                                            let path = format!("/delegation/{}", id);
                                                                            let body = UpdateStatusBody { status: status_str };
                                                                            match api_patch::<DelegatedWorkItem>(&path, &body).await {
                                                                                Ok(updated) => {
                                                                                    set_items.update(|list| {
                                                                                        if let Some(entry) = list.iter_mut().find(|e| e.id == updated.id) {
                                                                                            *entry = updated;
                                                                                        }
                                                                                    });
                                                                                }
                                                                                Err(e) => {
                                                                                    leptos::logging::warn!("Failed to update delegation status: {}", e);
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                >
                                                                    {s}
                                                                </button>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    </div>
                                                </div>
                                                <div class="delegation-item-actions">
                                                    {(has_subagent && on_open.is_some()).then(|| {
                                                        let cb = on_open.unwrap();
                                                        let sid = subagent_sid_btn.clone().unwrap();
                                                        view! {
                                                            <button
                                                                class="delegation-open-btn"
                                                                title="Open linked output"
                                                                on:click=move |_| cb.run(sid.clone())
                                                            >
                                                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                                    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                                                                    <polyline points="15 3 21 3 21 9" />
                                                                    <line x1="10" y1="14" x2="21" y2="3" />
                                                                </svg>
                                                            </button>
                                                        }
                                                    })}
                                                    {(has_session && !has_subagent && on_open.is_some()).then(|| {
                                                        let cb = on_open.unwrap();
                                                        let sid = session_sid_btn.clone().unwrap();
                                                        view! {
                                                            <button
                                                                class="delegation-open-btn"
                                                                title="Open linked session"
                                                                on:click=move |_| cb.run(sid.clone())
                                                            >
                                                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                                    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                                                                    <polyline points="15 3 21 3 21 9" />
                                                                    <line x1="10" y1="14" x2="21" y2="3" />
                                                                </svg>
                                                            </button>
                                                        }
                                                    })}
                                                    <button class="delegation-delete-btn" on:click=handle_delete>
                                                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                            <polyline points="3 6 5 6 21 6" />
                                                            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                                                        </svg>
                                                    </button>
                                                </div>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </div>
        </ModalOverlay>
    }
}
