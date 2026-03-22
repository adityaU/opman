//! Mobile explorer overlays — inline create, inline rename, confirm delete.

use leptos::prelude::*;

use crate::components::icons::{IconFilePlus, IconFolderPlus, IconPencil};

use super::types::ConfirmDeleteEntry;

/// Inline create row (new file / new folder).
pub fn render_inline_create(
    mobile_inline_create: ReadSignal<Option<String>>,
    set_mobile_inline_create: WriteSignal<Option<String>>,
    create_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    create_dir: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    current_path: ReadSignal<String>,
) -> impl IntoView {
    move || {
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
                    if kind == "file" {
                        cf(cp.clone(), v);
                    } else {
                        cd(cp.clone(), v);
                    }
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
    }
}

/// Inline rename row.
pub fn render_inline_rename(
    mobile_inline_rename: ReadSignal<Option<ConfirmDeleteEntry>>,
    set_mobile_inline_rename: WriteSignal<Option<ConfirmDeleteEntry>>,
    rename_entry: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String, bool)>>,
) -> impl IntoView {
    move || {
        let ir = mobile_inline_rename.get()?;
        let set_ir = set_mobile_inline_rename;
        let rename = rename_entry.clone();
        let (val, set_val) = signal(ir.name.clone());
        let path = ir.path.clone();
        let is_dir = ir.is_dir;

        let submit = {
            let rename = rename.clone();
            let path = path.clone();
            move || {
                let v = val.get_untracked().trim().to_string();
                if !v.is_empty() {
                    rename(path.clone(), v, is_dir);
                }
                set_ir.set(None);
            }
        };
        let submit2 = submit.clone();
        let submit3 = submit.clone();

        Some(view! {
            <div class="mobile-inline-rename">
                <IconPencil size=14 class="file-icon" />
                <input
                    class="explorer-inline-name-input"
                    placeholder="new name"
                    prop:value=move || val.get()
                    on:input=move |e| set_val.set(event_target_value(&e))
                    on:keydown=move |e| {
                        if e.key() == "Enter" { submit2(); }
                        else if e.key() == "Escape" { set_ir.set(None); }
                    }
                    on:blur=move |_| submit3()
                    autofocus=true
                />
            </div>
        })
    }
}

/// Confirm delete overlay bar.
pub fn render_confirm_delete(
    mobile_confirm_delete: ReadSignal<Option<ConfirmDeleteEntry>>,
    set_mobile_confirm_delete: WriteSignal<Option<ConfirmDeleteEntry>>,
    delete_file: send_wrapper::SendWrapper<super::actions::Fn1>,
    delete_dir: send_wrapper::SendWrapper<super::actions::Fn1>,
) -> impl IntoView {
    move || {
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
                    <crate::components::icons::IconX size=12 />
                </button>
            </div>
        })
    }
}

/// Trigger a browser download via a temporary `<a>` element.
pub fn trigger_download(url: &str) {
    use wasm_bindgen::JsCast as _;
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(el) = doc.create_element("a") else {
        return;
    };
    let a: web_sys::HtmlAnchorElement = el.unchecked_into();
    a.set_href(url);
    a.set_attribute("download", "").ok();
    a.set_attribute("style", "display:none").ok();
    let Some(body) = doc.body() else {
        return;
    };
    body.append_child(&a).ok();
    a.click();
    body.remove_child(&a).ok();
}
