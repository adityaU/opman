//! Sidebar overlay views: context menu, delete/remove-project confirmations.

use super::types::{ContextMenuState, DeleteConfirm, RemoveProjectConfirm};
use crate::components::icons::*;
use leptos::prelude::*;
use std::collections::HashSet;

// ── Helpers ─────────────────────────────────────────────────────────

/// Check if viewport is phone-width (matches `@media(max-width: 768px)`).
fn is_mobile() -> bool {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .map(|w| w <= 768.0)
        .unwrap_or(false)
}

/// Clamp menu position so it stays within the viewport. Returns `(left, top)`.
fn clamped_position(x: i32, y: i32) -> (i32, i32) {
    const MENU_W: i32 = 160;
    const MENU_H: i32 = 140;
    const PAD: i32 = 8;
    let (vw, vh) = viewport_size();
    (
        x.min(vw - MENU_W - PAD).max(PAD),
        y.min(vh - MENU_H - PAD).max(PAD),
    )
}

fn viewport_size() -> (i32, i32) {
    web_sys::window()
        .map(|w| {
            let vw = w
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(800.0) as i32;
            let vh = w
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(600.0) as i32;
            (vw, vh)
        })
        .unwrap_or((800, 600))
}

// ── Context menu ────────────────────────────────────────────────────

#[component]
pub fn SidebarContextMenu(
    ctx_menu: ReadSignal<Option<ContextMenuState>>,
    set_ctx_menu: WriteSignal<Option<ContextMenuState>>,
    pinned_sessions: ReadSignal<HashSet<String>>,
    open_sessions: ReadSignal<HashSet<String>>,
    toggle_pin: Callback<String>,
    toggle_open_session: Callback<String>,
    set_renaming_sid: WriteSignal<Option<String>>,
    set_rename_text: WriteSignal<String>,
    set_rename_original_title: WriteSignal<String>,
    set_delete_confirm: WriteSignal<Option<DeleteConfirm>>,
) -> impl IntoView {
    move || {
        let menu = ctx_menu.get()?;
        let mobile = is_mobile();
        let (sid_pin, sid_open, sid_open_label, sid_pin_label, sid_rename, sid_delete) = (
            menu.session_id.clone(),
            menu.session_id.clone(),
            menu.session_id.clone(),
            menu.session_id.clone(),
            menu.session_id.clone(),
            menu.session_id.clone(),
        );
        let (title_rename, title_delete) = (menu.session_title.clone(), menu.session_title.clone());
        let display_title = if menu.session_title.is_empty() {
            menu.session_id[..menu.session_id.len().min(16)].to_string()
        } else if menu.session_title.len() > 32 {
            format!("{}...", &menu.session_title[..29])
        } else {
            menu.session_title.clone()
        };
        let icon_sz: u32 = if mobile { 16 } else { 12 };
        let items = view! {
            <button
                class="sb-context-item"
                on:click=move |_| {
                    toggle_open_session.run(sid_open.clone());
                    set_ctx_menu.set(None);
                }
            >
                <IconLayers size=icon_sz />
                {move || {
                    if open_sessions.get().contains(&sid_open_label) {
                        "Remove from Open"
                    } else {
                        "Keep Open"
                    }
                }}
            </button>
            <button
                class="sb-context-item"
                on:click=move |_| {
                    toggle_pin.run(sid_pin.clone());
                    set_ctx_menu.set(None);
                }
            >
                <IconPin size=icon_sz />
                {move || {
                    if pinned_sessions.get().contains(&sid_pin_label) {
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
                <IconPencil size=icon_sz />
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
                <IconTrash2 size=icon_sz />
                "Delete"
            </button>
        };
        if mobile {
            // Mobile: bottom action-sheet with backdrop
            Some(
                view! {
                    <div class="sb-ctx-overlay" on:click=move |_| set_ctx_menu.set(None)>
                        <div
                            class="sb-context-menu sb-context-sheet"
                            on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()
                        >
                            <div class="sb-ctx-sheet-title">{display_title}</div>
                            {items}
                            <button
                                class="sb-context-item sb-ctx-cancel"
                                on:click=move |_| set_ctx_menu.set(None)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                }
                .into_any(),
            )
        } else {
            // Desktop: clamped fixed dropdown
            let (left, top) = clamped_position(menu.x, menu.y);
            Some(
                view! {
                    <div
                        class="sb-context-menu"
                        style:left=format!("{}px", left)
                        style:top=format!("{}px", top)
                        on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()
                    >
                        {items}
                    </div>
                }
                .into_any(),
            )
        }
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
