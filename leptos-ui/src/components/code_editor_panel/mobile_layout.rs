//! Mobile layout for the code editor panel.
//! Matches React `MobileLayout.tsx` — shows a flat file browser when no file
//! is active, and switches to the full editor view when a file is open.

use leptos::prelude::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

use crate::components::icons::{
    IconFile, IconFilePlus, IconFolder, IconFolderPlus, IconLoader2, IconMoreVertical, IconTrash2,
    IconUpload, IconX,
};
use crate::types::api::FileEntry;

use super::actions::Fn1;
use super::state::{BreadcrumbEntry, EditorState};
use super::types::{format_size, ConfirmDeleteEntry};

/// Render the mobile file browser (flat entry list with breadcrumbs + actions dropdown).
/// Returns `None` when a file is open (caller should render the editor instead).
pub fn render_mobile_browser(
    s: &EditorState,
    breadcrumbs: Memo<Vec<BreadcrumbEntry>>,
    load_dir: send_wrapper::SendWrapper<Fn1>,
    open_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    create_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    create_dir: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    delete_file: send_wrapper::SendWrapper<Fn1>,
    delete_dir: send_wrapper::SendWrapper<Fn1>,
    upload: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, web_sys::FileList)>>,
) -> impl IntoView {
    let entries = s.entries;
    let explorer_loading = s.explorer_loading;
    let current_path = s.current_path;
    let mobile_actions_open = s.mobile_actions_open;
    let set_mobile_actions_open = s.set_mobile_actions_open;
    let mobile_inline_create = s.mobile_inline_create;
    let set_mobile_inline_create = s.set_mobile_inline_create;
    let mobile_confirm_delete = s.mobile_confirm_delete;
    let set_mobile_confirm_delete = s.set_mobile_confirm_delete;

    // Close dropdown on outside click
    let toggle_ref = NodeRef::<leptos::html::Button>::new();
    {
        let set_open = set_mobile_actions_open;
        Effect::new(move |_| {
            if !mobile_actions_open.get() {
                return;
            }
            let cb = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                let target = e.target();
                if let Some(btn) = toggle_ref.get() {
                    let el: &web_sys::Node = btn.as_ref();
                    if let Some(ref t) = target {
                        if el.contains(Some(t.unchecked_ref())) {
                            return;
                        }
                    }
                }
                set_open.set(false);
            });
            let _ = web_sys::window().and_then(|w| w.document()).map(|doc| {
                let _ = doc.add_event_listener_with_callback_and_add_event_listener_options(
                    "mousedown",
                    cb.as_ref().unchecked_ref(),
                    web_sys::AddEventListenerOptions::new()
                        .capture(true)
                        .once(true),
                );
            });
            cb.forget();
        });
    }

    let upload_c = upload.clone();
    let load_dir_filelist = load_dir.clone();
    let open_file_filelist = open_file.clone();

    view! {
        <div class="code-editor-panel">
            // Toolbar with breadcrumbs + actions toggle
            <div class="code-editor-toolbar">
                <div class="code-editor-breadcrumbs">
                    {move || {
                        let crumbs = breadcrumbs.get();
                        let ld = load_dir.clone();
                        crumbs.into_iter().enumerate().map(|(i, crumb)| {
                            let ld = ld.clone();
                            let path = crumb.path.clone();
                            view! {
                                {(i > 0).then(|| view! { <span class="breadcrumb-sep">"/"</span> })}
                                <button class="breadcrumb-link" on:click=move |_| ld(path.clone())>
                                    {crumb.label}
                                </button>
                            }
                        }).collect::<Vec<_>>()
                    }}
                </div>
                <div class="mobile-explorer-actions-toggle">
                    <button
                        class="explorer-hdr-btn"
                        title="File actions"
                        node_ref=toggle_ref
                        on:click=move |_| set_mobile_actions_open.update(|v| *v = !*v)
                    ><IconMoreVertical size=14 /></button>
                    {move || mobile_actions_open.get().then(|| {
                        let set_ic = set_mobile_inline_create;
                        let set_open = set_mobile_actions_open;
                        let upload = upload_c.clone();
                        let cp = current_path.get_untracked();
                        view! {
                            <div class="mobile-explorer-actions-dropdown">
                                <button class="mobile-action-item" on:click=move |_| {
                                    set_ic.set(Some("file".into()));
                                    set_open.set(false);
                                }><IconFilePlus size=13 />" New File"</button>
                                <button class="mobile-action-item" on:click=move |_| {
                                    set_ic.set(Some("dir".into()));
                                    set_open.set(false);
                                }><IconFolderPlus size=13 />" New Folder"</button>
                                <button class="mobile-action-item" on:click={
                                    let cp = cp.clone();
                                    move |_| {
                                        set_open.set(false);
                                        trigger_upload();
                                    }
                                }><IconUpload size=13 />" Upload"</button>
                            </div>
                        }
                    })}
                </div>
                <input id="mobile-upload-input" type="file" multiple=true style="display:none;"
                    on:change={
                        let upload = upload.clone();
                        move |e| {
                            let target: web_sys::HtmlInputElement = e.target().unwrap().unchecked_into();
                            if let Some(files) = target.files() {
                                if files.length() > 0 {
                                    upload(current_path.get_untracked(), files);
                                }
                            }
                            target.set_value("");
                        }
                    }
                />
            </div>

            // Inline create
            {move || {
                let kind = mobile_inline_create.get()?;
                let set_ic = set_mobile_inline_create;
                let cf = create_file.clone();
                let cd = create_dir.clone();
                let cp = current_path.get_untracked();
                let is_dir = kind == "dir";
                let placeholder = if is_dir { "folder name" } else { "filename" };
                let (val, set_val) = signal(String::new());
                let submit = {
                    let cf = cf.clone();
                    let cd = cd.clone();
                    let cp = cp.clone();
                    let kind = kind.clone();
                    move || {
                        let v = val.get_untracked().trim().to_string();
                        if !v.is_empty() {
                            if kind == "file" { cf(cp.clone(), v); } else { cd(cp.clone(), v); }
                        }
                        set_ic.set(None);
                    }
                };
                let submit2 = submit.clone();
                let submit3 = submit.clone();
                Some(view! {
                    <div class="mobile-inline-create">
                        {if is_dir {
                            view! { <IconFolderPlus size=14 class="file-icon folder-icon" /> }.into_any()
                        } else {
                            view! { <IconFilePlus size=14 class="file-icon" /> }.into_any()
                        }}
                        <input
                            class="explorer-inline-name-input"
                            placeholder=placeholder
                            prop:value=move || val.get()
                            on:input=move |e| set_val.set(event_target_value(&e))
                            on:keydown=move |e| {
                                if e.key() == "Enter" { submit2(); }
                                else if e.key() == "Escape" { set_ic.set(None); }
                            }
                            on:blur=move |_| submit3()
                            autofocus=true
                        />
                    </div>
                })
            }}

            // Confirm delete overlay
            {move || {
                let cd = mobile_confirm_delete.get()?;
                let set_cd = set_mobile_confirm_delete;
                let df = delete_file.clone();
                let dd = delete_dir.clone();
                let label = if cd.is_dir {
                    format!("Delete folder {}?", cd.name)
                } else {
                    format!("Delete {}?", cd.name)
                };
                let path = cd.path.clone();
                let is_dir = cd.is_dir;
                Some(view! {
                    <div class="mobile-confirm-delete">
                        <span>{label}</span>
                        <button class="explorer-confirm-yes" on:click=move |_| {
                            if is_dir { dd(path.clone()); } else { df(path.clone()); }
                            set_cd.set(None);
                        }>"Delete"</button>
                        <button class="explorer-confirm-no" on:click=move |_| set_cd.set(None)>
                            <IconX size=12 />
                        </button>
                    </div>
                })
            }}

            // File list
            <div class="code-editor-filelist">
                {move || {
                    if explorer_loading.get() {
                        return view! {
                            <div class="code-editor-loading">
                                <IconLoader2 size=20 class="spin" />
                                <span>"Loading..."</span>
                            </div>
                        }.into_any();
                    }
                    let items = entries.get();
                    if items.is_empty() {
                        return view! { <div class="code-editor-empty">"Empty directory"</div> }.into_any();
                    }
                    let of = open_file_filelist.clone();
                    let ld = load_dir_filelist.clone();
                    items.into_iter().map(|entry| {
                        let of = of.clone();
                        let ld = ld.clone();
                        let path = entry.path.clone();
                        let name = entry.name.clone();
                        let is_dir = entry.is_dir;
                        let size = entry.size;
                        let del_name = entry.name.clone();
                        let del_path = entry.path.clone();
                        view! {
                            <div class="code-editor-file-entry-row">
                                <button
                                    class="code-editor-file-entry"
                                    on:click=move |_| {
                                        if is_dir { ld(path.clone()); }
                                        else { of(path.clone(), name.clone()); }
                                    }
                                >
                                    {if entry.is_dir {
                                        view! { <IconFolder size=14 class="file-icon folder-icon" /> }.into_any()
                                    } else {
                                        view! { <IconFile size=14 class="file-icon" /> }.into_any()
                                    }}
                                    <span class="file-name">{entry.name.clone()}</span>
                                    {(!entry.is_dir).then(|| {
                                        view! { <span class="file-size">{format_size(size)}</span> }
                                    })}
                                </button>
                                <button
                                    class="mobile-entry-delete-btn"
                                    title=format!("Delete {}", del_name)
                                    on:click=move |_| set_mobile_confirm_delete.set(Some(
                                        ConfirmDeleteEntry { path: del_path.clone(), name: del_name.clone(), is_dir: entry.is_dir }
                                    ))
                                ><IconTrash2 size=12 /></button>
                            </div>
                        }
                    }).collect::<Vec<_>>().into_any()
                }}
            </div>
        </div>
    }
}

/// Trigger the hidden file upload input.
fn trigger_upload() {
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Some(el) = doc.get_element_by_id("mobile-upload-input") {
            let input: web_sys::HtmlInputElement = el.unchecked_into();
            input.click();
        }
    }
}
