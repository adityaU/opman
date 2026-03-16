//! Sidebar overlay views: context menu, delete/remove-project confirmations.

use std::collections::HashSet;

use leptos::prelude::*;

use crate::components::icons::*;

use super::types::{ContextMenuState, DeleteConfirm, RemoveProjectConfirm};

// ── Context menu ────────────────────────────────────────────────────

#[component]
pub fn SidebarContextMenu(
    ctx_menu: ReadSignal<Option<ContextMenuState>>,
    set_ctx_menu: WriteSignal<Option<ContextMenuState>>,
    pinned_sessions: ReadSignal<HashSet<String>>,
    toggle_pin: Callback<String>,
    set_renaming_sid: WriteSignal<Option<String>>,
    set_rename_text: WriteSignal<String>,
    set_rename_original_title: WriteSignal<String>,
    set_delete_confirm: WriteSignal<Option<DeleteConfirm>>,
) -> impl IntoView {
    move || {
        ctx_menu.get().map(|menu| {
            let sid_pin = menu.session_id.clone();
            let sid_rename = menu.session_id.clone();
            let title_rename = menu.session_title.clone();
            let sid_delete = menu.session_id.clone();
            let title_delete = menu.session_title.clone();
            view! {
                <div
                    class="sb-context-menu"
                    style:left=format!("{}px", menu.x)
                    style:top=format!("{}px", menu.y)
                    on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()
                >
                    <button
                        class="sb-context-item"
                        on:click=move |_| {
                            let _is_pinned = pinned_sessions.get_untracked().contains(&sid_pin);
                            toggle_pin.run(sid_pin.clone());
                            set_ctx_menu.set(None);
                        }
                    >
                        <IconPin size=12 />
                        {move || {
                            if pinned_sessions.get().contains(&menu.session_id) {
                                "Unpin"
                            } else {
                                "Pin to Top"
                            }
                        }}
                    </button>
                    <button
                        class="sb-context-item"
                        on:click=move |_| {
                            set_rename_text.set(title_rename.clone());
                            set_rename_original_title.set(title_rename.clone());
                            set_renaming_sid.set(Some(sid_rename.clone()));
                            set_ctx_menu.set(None);
                        }
                    >
                        <IconPencil size=12 />
                        "Rename"
                    </button>
                    <button
                        class="sb-context-item sb-context-danger"
                        on:click=move |_| {
                            set_delete_confirm.set(Some(DeleteConfirm {
                                session_id: sid_delete.clone(),
                                session_title: title_delete.clone(),
                            }));
                            set_ctx_menu.set(None);
                        }
                    >
                        <IconTrash2 size=12 />
                        "Delete"
                    </button>
                </div>
            }
        })
    }
}

// ── Delete session confirmation ─────────────────────────────────────

#[component]
pub fn DeleteSessionModal(
    delete_confirm: ReadSignal<Option<DeleteConfirm>>,
    set_delete_confirm: WriteSignal<Option<DeleteConfirm>>,
    delete_loading: ReadSignal<bool>,
    do_delete: Callback<String>,
) -> impl IntoView {
    move || {
        delete_confirm.get().map(|confirm| {
            let sid = confirm.session_id.clone();
            let title = if confirm.session_title.is_empty() {
                confirm.session_id[..confirm.session_id.len().min(12)].to_string()
            } else {
                confirm.session_title.clone()
            };
            view! {
                <div
                    class="sb-modal-overlay"
                    on:click=move |_| {
                        if !delete_loading.get_untracked() {
                            set_delete_confirm.set(None);
                        }
                    }
                >
                    <div
                        class="sb-modal"
                        on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()
                    >
                        <div class="sb-modal-title">"Delete Session"</div>
                        <div class="sb-modal-body">
                            "Are you sure you want to delete "
                            <strong>{title}</strong>
                            "? This action cannot be undone."
                        </div>
                        <div class="sb-modal-actions">
                            <button
                                class="sb-modal-btn sb-modal-cancel"
                                on:click=move |_| set_delete_confirm.set(None)
                                disabled=move || delete_loading.get()
                            >
                                "Cancel"
                            </button>
                            <button
                                class="sb-modal-btn sb-modal-danger"
                                on:click={
                                    let sid = sid.clone();
                                    move |_| do_delete.run(sid.clone())
                                }
                                disabled=move || delete_loading.get()
                            >
                                {move || if delete_loading.get() { "Deleting..." } else { "Delete" }}
                            </button>
                        </div>
                    </div>
                </div>
            }
        })
    }
}

// ── Remove project confirmation ─────────────────────────────────────

#[component]
pub fn RemoveProjectModal(
    remove_project_confirm: ReadSignal<Option<RemoveProjectConfirm>>,
    set_remove_project_confirm: WriteSignal<Option<RemoveProjectConfirm>>,
    remove_project_loading: ReadSignal<bool>,
    do_remove_project: Callback<usize>,
) -> impl IntoView {
    move || {
        remove_project_confirm.get().map(|confirm| {
            let pidx = confirm.project_idx;
            let name = confirm.project_name.clone();
            view! {
                <div
                    class="sb-modal-overlay"
                    on:click=move |_| {
                        if !remove_project_loading.get_untracked() {
                            set_remove_project_confirm.set(None);
                        }
                    }
                >
                    <div
                        class="sb-modal"
                        on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()
                    >
                        <div class="sb-modal-title">"Remove Project"</div>
                        <div class="sb-modal-body">
                            "Are you sure you want to remove "
                            <strong>{name}</strong>
                            " from the workspace? This will not delete any files on disk."
                        </div>
                        <div class="sb-modal-actions">
                            <button
                                class="sb-modal-btn sb-modal-cancel"
                                on:click=move |_| set_remove_project_confirm.set(None)
                                disabled=move || remove_project_loading.get()
                            >
                                "Cancel"
                            </button>
                            <button
                                class="sb-modal-btn sb-modal-danger"
                                on:click=move |_| do_remove_project.run(pidx)
                                disabled=move || remove_project_loading.get()
                            >
                                {move || if remove_project_loading.get() { "Removing..." } else { "Remove" }}
                            </button>
                        </div>
                    </div>
                </div>
            }
        })
    }
}
