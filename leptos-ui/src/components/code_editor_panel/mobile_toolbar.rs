//! Mobile explorer toolbar — breadcrumbs + global file-action dropdown.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::icons::{
    IconFilePlus, IconFolderPlus, IconLoader2, IconMoreVertical, IconRefreshCw, IconUpload,
};

use super::actions::{Fn0, Fn1};
use super::state::BreadcrumbEntry;

/// Render the breadcrumb bar with a 3-dot global-actions dropdown.
pub fn render_toolbar(
    breadcrumbs: Memo<Vec<BreadcrumbEntry>>,
    load_dir: send_wrapper::SendWrapper<Fn1>,
    current_path: ReadSignal<String>,
    file_action_busy: ReadSignal<bool>,
    mobile_actions_open: ReadSignal<bool>,
    set_mobile_actions_open: WriteSignal<bool>,
    set_mobile_inline_create: WriteSignal<Option<String>>,
    reload_root: send_wrapper::SendWrapper<Fn0>,
    upload: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, web_sys::FileList)>>,
    toggle_ref: NodeRef<leptos::html::Button>,
    dropdown_ref: NodeRef<leptos::html::Div>,
) -> impl IntoView {
    view! {
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
                {move || file_action_busy.get().then(|| view! {
                    <span style="opacity:0.6;margin-right:4px;display:inline-flex;">
                        <IconLoader2 size=14 class="spin" />
                    </span>
                })}
                <button
                    class="explorer-hdr-btn"
                    title="File actions"
                    node_ref=toggle_ref
                    on:click=move |_| set_mobile_actions_open.update(|v| *v = !*v)
                ><IconMoreVertical size=14 /></button>
                {move || mobile_actions_open.get().then(|| {
                    let set_ic = set_mobile_inline_create;
                    let set_open = set_mobile_actions_open;
                    let reload = reload_root.clone();
                    view! {
                        <div class="mobile-explorer-actions-dropdown" node_ref=dropdown_ref>
                            <button class="mobile-action-item" on:click=move |_| {
                                set_ic.set(Some("file".into()));
                                set_open.set(false);
                            }><IconFilePlus size=13 />" New File"</button>
                            <button class="mobile-action-item" on:click=move |_| {
                                set_ic.set(Some("dir".into()));
                                set_open.set(false);
                            }><IconFolderPlus size=13 />" New Folder"</button>
                            <button class="mobile-action-item" on:click=move |_| {
                                set_open.set(false);
                                trigger_upload();
                            }><IconUpload size=13 />" Upload"</button>
                            <button class="mobile-action-item" on:click=move |_| {
                                set_open.set(false);
                                reload();
                            }><IconRefreshCw size=13 />" Reload"</button>
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
    }
}

/// Trigger the hidden file upload input.
fn trigger_upload() {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(el) = doc.get_element_by_id("mobile-upload-input") else {
        return;
    };
    let input: web_sys::HtmlInputElement = el.unchecked_into();
    input.click();
}
