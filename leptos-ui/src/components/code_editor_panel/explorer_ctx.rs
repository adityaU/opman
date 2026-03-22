//! Explorer tree context and inline UI helpers (create input, confirm delete, rename).

use leptos::prelude::*;

use crate::components::icons::{IconFilePlus, IconFolderPlus, IconPencil, IconX};
use crate::types::api::FileEntry;

use super::types::ConfirmDeleteEntry;

/// Shared context for explorer tree callbacks, avoiding prop-drilling of closures.
/// All callbacks use `SendWrapper<Rc<...>>` so they are Send-safe in Leptos view closures.
#[derive(Clone)]
pub struct ExplorerTreeCtx {
    pub expanded_dirs: ReadSignal<std::collections::HashSet<String>>,
    pub dir_children: ReadSignal<std::collections::HashMap<String, Vec<FileEntry>>>,
    pub loading_dirs: ReadSignal<std::collections::HashSet<String>>,
    pub active_file_path: ReadSignal<Option<String>>,
    pub inline_create: ReadSignal<Option<(String, String)>>,
    pub set_inline_create: WriteSignal<Option<(String, String)>>,
    pub inline_confirm_delete: ReadSignal<Option<ConfirmDeleteEntry>>,
    pub set_inline_confirm_delete: WriteSignal<Option<ConfirmDeleteEntry>>,
    pub inline_rename: ReadSignal<Option<ConfirmDeleteEntry>>,
    pub set_inline_rename: WriteSignal<Option<ConfirmDeleteEntry>>,
    pub explorer_ctx_menu: ReadSignal<Option<String>>,
    pub set_explorer_ctx_menu: WriteSignal<Option<String>>,
    pub toggle_dir: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String)>>,
    pub open_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    pub handle_create_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    pub handle_create_dir: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    pub handle_delete_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String)>>,
    pub handle_delete_dir: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String)>>,
    pub handle_reload_dir: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String)>>,
    pub handle_reload_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String)>>,
    pub handle_rename: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String, bool)>>,
}

/// Inline create input — matches React `InlineCreateInput`.
pub fn render_inline_create_input(
    create_type: &str,
    depth: u32,
    on_submit: std::rc::Rc<dyn Fn(String)>,
    on_cancel: std::rc::Rc<dyn Fn()>,
) -> impl IntoView {
    let (value, set_value) = signal(String::new());
    let padding = format!("{}px", 8 + depth * 14);
    let placeholder = if create_type == "dir" {
        "folder name"
    } else {
        "filename"
    };
    let is_dir = create_type == "dir";
    let submit_1 = send_wrapper::SendWrapper::new(on_submit.clone());
    let cancel_1 = send_wrapper::SendWrapper::new(on_cancel.clone());
    let submit_2 = send_wrapper::SendWrapper::new(on_submit.clone());
    let cancel_2 = send_wrapper::SendWrapper::new(on_cancel.clone());

    let node = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| {
        if let Some(el) = node.get() {
            let _ = el.focus();
        }
    });

    view! {
        <div class="explorer-inline-input" style:padding-left=padding>
            {if is_dir {
                view! { <IconFolderPlus size=13 class="file-icon folder-icon" /> }.into_any()
            } else {
                view! { <IconFilePlus size=13 class="file-icon" /> }.into_any()
            }}
            <input
                class="explorer-inline-name-input"
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=move |e| set_value.set(event_target_value(&e))
                on:keydown=move |e| {
                    if e.key() == "Enter" {
                        let v = value.get_untracked().trim().to_string();
                        if !v.is_empty() { submit_1(v); } else { cancel_1(); }
                    } else if e.key() == "Escape" {
                        cancel_2();
                    }
                }
                on:blur=move |_| {
                    let v = value.get_untracked().trim().to_string();
                    if !v.is_empty() { submit_2(v); } else { on_cancel(); }
                }
                node_ref=node
            />
        </div>
    }
}

/// Inline confirm delete — matches React `ConfirmDeleteInline`.
pub fn render_confirm_delete_inline(
    path: &str,
    is_dir: bool,
    depth: u32,
    on_confirm: std::rc::Rc<dyn Fn()>,
    on_cancel: std::rc::Rc<dyn Fn()>,
) -> impl IntoView {
    let name = path.rsplit('/').next().unwrap_or(path).to_string();
    let padding = format!("{}px", 8 + depth * 14);
    let label = if is_dir {
        format!("Delete folder {}?", name)
    } else {
        format!("Delete {}?", name)
    };
    let confirm_sw = send_wrapper::SendWrapper::new(on_confirm);
    let cancel_sw = send_wrapper::SendWrapper::new(on_cancel);
    view! {
        <div class="explorer-confirm-delete" style:padding-left=padding>
            <span class="explorer-confirm-text">{label}</span>
            <button class="explorer-confirm-yes" on:click=move |_| confirm_sw()>"Delete"</button>
            <button class="explorer-confirm-no" on:click=move |_| cancel_sw()>
                <IconX size=12 />
            </button>
        </div>
    }
}

/// Inline rename input — shows a text input pre-filled with the current name.
pub fn render_inline_rename_input(
    current_name: &str,
    is_dir: bool,
    depth: u32,
    on_submit: std::rc::Rc<dyn Fn(String)>,
    on_cancel: std::rc::Rc<dyn Fn()>,
) -> impl IntoView {
    let (value, set_value) = signal(current_name.to_string());
    let padding = format!("{}px", 8 + depth * 14);
    let submit_1 = send_wrapper::SendWrapper::new(on_submit.clone());
    let cancel_1 = send_wrapper::SendWrapper::new(on_cancel.clone());
    let submit_2 = send_wrapper::SendWrapper::new(on_submit.clone());
    let cancel_2 = send_wrapper::SendWrapper::new(on_cancel.clone());

    let node = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| {
        if let Some(el) = node.get() {
            let _ = el.focus();
            // Select filename without extension for files
            let v = value.get_untracked();
            if !is_dir {
                if let Some(dot_pos) = v.rfind('.') {
                    let _ = el.set_selection_range(0, dot_pos as u32);
                } else {
                    let _ = el.select();
                }
            } else {
                let _ = el.select();
            }
        }
    });

    view! {
        <div class="explorer-inline-input explorer-inline-rename" style:padding-left=padding>
            <IconPencil size=13 class="file-icon" />
            <input
                class="explorer-inline-name-input"
                placeholder="new name"
                prop:value=move || value.get()
                on:input=move |e| set_value.set(event_target_value(&e))
                on:keydown=move |e| {
                    if e.key() == "Enter" {
                        let v = value.get_untracked().trim().to_string();
                        if !v.is_empty() { submit_1(v); } else { cancel_1(); }
                    } else if e.key() == "Escape" {
                        cancel_2();
                    }
                }
                on:blur=move |_| {
                    let v = value.get_untracked().trim().to_string();
                    if !v.is_empty() { submit_2(v); } else { on_cancel(); }
                }
                node_ref=node
            />
        </div>
    }
}
