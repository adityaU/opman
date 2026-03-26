//! DelegationBoardModal item row — status buttons, delete, open-session.

use leptos::prelude::*;
use serde::Serialize;
use crate::api::client::{api_delete, api_patch};
use crate::types::api::DelegatedWorkItem;

#[derive(Serialize)]
struct UpdateStatusBody { status: String }

const STATUSES: &[&str] = &["planned", "running", "completed"];

#[component]
pub fn DelegationItemRow(
    item: DelegatedWorkItem,
    is_selected: bool,
    idx: usize,
    set_items: WriteSignal<Vec<DelegatedWorkItem>>,
    on_hover: Callback<usize>,
    #[prop(into)] on_open_session: Option<Callback<String>>,
) -> impl IntoView {
    let item_id = item.id.clone();
    let item_id_delete = item.id.clone();
    let title = item.title.clone();
    let scope_text = item.scope.clone();
    let assignee_text = item.assignee.clone();
    let status = item.status.clone();
    let subagent_sid = item.subagent_session_id.clone();
    let session_sid = item.session_id.clone();

    let handle_delete = {
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

    let on_open = on_open_session;
    let subagent_sid_btn = subagent_sid.clone();
    let session_sid_btn = session_sid.clone();
    let has_subagent = subagent_sid.is_some();
    let has_session = session_sid.is_some();
    let status_clone = status.clone();

    view! {
        <div
            class=if is_selected { "delegation-item selected" } else { "delegation-item" }
            attr:data-deleg-idx=idx
            on:mouseenter=move |_| on_hover.run(idx)
        >
            <div class="delegation-item-main">
                <div class="delegation-item-title">{title}</div>
                <div class="delegation-item-scope">{scope_text}</div>
                <div class="delegation-item-meta">
                    <span>{assignee_text}</span>
                    <span>{status_clone}</span>
                    {subagent_sid.map(|sid| view! { <span>"subagent " {sid}</span> })}
                </div>
                <div class="delegation-status-actions">
                    {STATUSES.iter().map(|&s| {
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
                                        if let Ok(updated) = api_patch::<DelegatedWorkItem>(&path, &body).await {
                                            set_items.update(|list| {
                                                if let Some(e) = list.iter_mut().find(|e| e.id == updated.id) { *e = updated; }
                                            });
                                        }
                                    });
                                }
                            >{s}</button>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            </div>
            <div class="delegation-item-actions">
                {(has_subagent && on_open.is_some()).then(|| {
                    let cb = on_open.unwrap();
                    let sid = subagent_sid_btn.clone().unwrap();
                    view! {
                        <button class="delegation-open-btn" title="Open linked output"
                            on:click=move |_| cb.run(sid.clone())
                        ><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                            <polyline points="15 3 21 3 21 9" /><line x1="10" y1="14" x2="21" y2="3" />
                        </svg></button>
                    }
                })}
                {(has_session && !has_subagent && on_open.is_some()).then(|| {
                    let cb = on_open.unwrap();
                    let sid = session_sid_btn.clone().unwrap();
                    view! {
                        <button class="delegation-open-btn" title="Open linked session"
                            on:click=move |_| cb.run(sid.clone())
                        ><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                            <polyline points="15 3 21 3 21 9" /><line x1="10" y1="14" x2="21" y2="3" />
                        </svg></button>
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
}
