//! MemoryModal item row — view mode + inline edit mode, with CRUD actions.

use leptos::prelude::*;
use crate::api::client::{api_delete, api_patch};
use crate::types::api::PersonalMemoryItem;
use crate::components::icons::*;
use super::helpers::{
    describe_scope, format_relative_date, format_scope,
    UpdateMemoryBody, SCOPE_OPTIONS,
};
use crate::types::api::ProjectInfo;

#[component]
pub fn MemoryItemRow(
    item: PersonalMemoryItem,
    is_selected: bool,
    idx: usize,
    projects: Vec<ProjectInfo>,
    active_project_index: usize,
    active_session_id: Option<String>,
    set_items: WriteSignal<Vec<PersonalMemoryItem>>,
    editing_id: ReadSignal<Option<String>>,
    set_editing_id: WriteSignal<Option<String>>,
    editing_label: ReadSignal<String>,
    set_editing_label: WriteSignal<String>,
    editing_content: ReadSignal<String>,
    set_editing_content: WriteSignal<String>,
    editing_scope: ReadSignal<String>,
    set_editing_scope: WriteSignal<String>,
    on_hover: Callback<usize>,
) -> impl IntoView {
    let item_id = item.id.clone();
    let item_id_edit = item.id.clone();
    let item_id_cancel = item.id.clone();
    let item_id_edit_btn = item.id.clone();
    let item_id_del_btn = item.id.clone();
    let item_id_delete = item.id.clone();
    let label_display = item.label.clone();
    let content_display = item.content.clone();
    let desc = describe_scope(&item, &projects);
    let updated = format!("updated {}", format_relative_date(&item.updated_at));
    let start_edit_id = item.id.clone();
    let start_edit_label = item.label.clone();
    let start_edit_content = item.content.clone();
    let start_edit_scope = item.scope.clone();

    let handle_save = move |_: web_sys::MouseEvent| {
        let eid = editing_id.get_untracked();
        let el = editing_label.get_untracked();
        let ec = editing_content.get_untracked();
        let es = editing_scope.get_untracked();
        if eid.is_none() || el.trim().is_empty() || ec.trim().is_empty() {
            return;
        }
        let eid = eid.unwrap();
        let pi = active_project_index;
        let sid = active_session_id.clone();

        leptos::task::spawn_local(async move {
            let body = UpdateMemoryBody {
                label: el.trim().to_string(),
                content: ec.trim().to_string(),
                scope: es.clone(),
                project_index: if es == "project" || es == "session" {
                    Some(pi)
                } else {
                    None
                },
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
    };

    view! {
        <div
            class=if is_selected { "memory-item selected" } else { "memory-item" }
            attr:data-mem-idx=idx
            on:mouseenter=move |_| on_hover.run(idx)
        >
            <div class="memory-item-main">
                {move || {
                    if editing_id.get().as_ref() == Some(&item_id_edit) {
                        view! {
                            <div>
                                <input class="memory-input"
                                    prop:value=move || editing_label.get()
                                    on:input=move |ev| set_editing_label.set(event_target_value(&ev)) />
                                <select class="memory-select"
                                    prop:value=move || editing_scope.get()
                                    on:change=move |ev| set_editing_scope.set(event_target_value(&ev))
                                >
                                    {SCOPE_OPTIONS.iter().map(|opt| {
                                        let val = opt.to_string();
                                        let display = format_scope(opt);
                                        view! { <option value=val>{display}</option> }
                                    }).collect::<Vec<_>>()}
                                </select>
                                <textarea class="memory-textarea" rows="3"
                                    prop:value=move || editing_content.get()
                                    on:input=move |ev| set_editing_content.set(event_target_value(&ev)) />
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div>
                                <div class="memory-item-label">{label_display.clone()}</div>
                                <div class="memory-item-content">{content_display.clone()}</div>
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
                // Save + Cancel (editing mode)
                <button class="memory-edit-btn"
                    style=move || if editing_id.get().as_ref() == Some(&item_id) { "" } else { "display:none" }
                    on:click=handle_save aria-label="Save"
                >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                        stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
                        <polyline points="17 21 17 13 7 13 7 21" />
                        <polyline points="7 3 7 8 15 8" />
                    </svg>
                </button>
                <button class="memory-delete-btn"
                    style=move || if editing_id.get().as_ref() == Some(&item_id_cancel) { "" } else { "display:none" }
                    on:click=move |_: web_sys::MouseEvent| set_editing_id.set(None)
                    aria-label="Cancel"
                ><IconX size=14 /></button>
                // Edit + Delete (view mode)
                <button class="memory-edit-btn"
                    style=move || if editing_id.get().as_ref() != Some(&item_id_edit_btn) { "" } else { "display:none" }
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
                ><IconPenSquare size=14 /></button>
                <button class="memory-delete-btn"
                    style=move || if editing_id.get().as_ref() != Some(&item_id_del_btn) { "" } else { "display:none" }
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
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                        stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="3 6 5 6 21 6" />
                        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                </button>
            </div>
        </div>
    }
}
