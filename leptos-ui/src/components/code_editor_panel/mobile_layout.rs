//! Mobile layout — flat file browser when no file is active.
//! Uses swipe-to-reveal for per-entry actions (rename, download, delete).

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::icons::{
    IconDownload, IconFile, IconFolder, IconLoader2, IconPencil, IconTrash2,
};
use crate::hooks::use_swipe_reveal::{use_swipe_reveal, SwipeConfig};

use super::actions::{Fn0, Fn1};
use super::mobile_overlays::{
    render_confirm_delete, render_inline_create, render_inline_rename, trigger_download,
};
use super::mobile_toolbar::render_toolbar;
use super::state::{BreadcrumbEntry, EditorState};
use super::types::{format_size, ConfirmDeleteEntry};

/// Width of 3 swipe action buttons (34px each) + gaps + tray padding.
const SWIPE_ACTIONS_WIDTH: f64 = 128.0;

/// Render the mobile file browser (flat entry list with breadcrumbs + swipe actions).
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
    reload_root: send_wrapper::SendWrapper<Fn0>,
    rename_entry: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String, bool)>>,
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
    let mobile_inline_rename = s.mobile_inline_rename;
    let set_mobile_inline_rename = s.set_mobile_inline_rename;
    let file_action_busy = s.file_action_busy;

    // Close toolbar dropdown on outside click
    let toggle_ref = NodeRef::<leptos::html::Button>::new();
    let dropdown_ref = NodeRef::<leptos::html::Div>::new();
    install_outside_click_close(
        mobile_actions_open,
        set_mobile_actions_open,
        toggle_ref,
        dropdown_ref,
    );

    let load_dir_filelist = load_dir.clone();
    let open_file_filelist = open_file.clone();
    let delete_file_list = delete_file.clone();
    let delete_dir_list = delete_dir.clone();

    view! {
        <div class="code-editor-panel">
            {render_toolbar(
                breadcrumbs, load_dir, current_path, file_action_busy,
                mobile_actions_open, set_mobile_actions_open,
                set_mobile_inline_create, reload_root, upload,
                toggle_ref, dropdown_ref,
            )}
            {render_inline_create(mobile_inline_create, set_mobile_inline_create, create_file, create_dir, current_path)}
            {render_inline_rename(mobile_inline_rename, set_mobile_inline_rename, rename_entry)}
            {render_confirm_delete(mobile_confirm_delete, set_mobile_confirm_delete, delete_file, delete_dir)}
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
                    let df = delete_file_list.clone();
                    let dd = delete_dir_list.clone();
                    items.into_iter().map(|entry| {
                        render_file_entry(
                            &entry, &of, &ld,
                            set_mobile_inline_rename, set_mobile_confirm_delete,
                            &df, &dd,
                        )
                    }).collect::<Vec<_>>().into_any()
                }}
            </div>
        </div>
    }
}

// ── Outside-click handler ──────────────────────────────────────────

fn install_outside_click_close(
    is_open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    toggle_ref: NodeRef<leptos::html::Button>,
    dropdown_ref: NodeRef<leptos::html::Div>,
) {
    use wasm_bindgen::closure::Closure;
    Effect::new(move |_| {
        if !is_open.get() {
            return;
        }
        let cb = Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
            if let Some(ref t) = e.target() {
                let t: &web_sys::Node = t.unchecked_ref();
                if let Some(btn) = toggle_ref.get() {
                    if (<_ as AsRef<web_sys::Node>>::as_ref(&btn)).contains(Some(t)) {
                        return;
                    }
                }
                if let Some(dd) = dropdown_ref.get() {
                    if (<_ as AsRef<web_sys::Node>>::as_ref(&dd)).contains(Some(t)) {
                        return;
                    }
                }
            }
            set_open.set(false);
        });
        let _ = web_sys::window().and_then(|w| w.document()).map(|doc| {
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_capture(true);
            opts.set_once(true);
            let _ = doc.add_event_listener_with_callback_and_add_event_listener_options(
                "mousedown",
                cb.as_ref().unchecked_ref(),
                &opts,
            );
        });
        cb.forget();
    });
}

// ── Single file/dir entry with swipe-to-reveal ─────────────────────

fn render_file_entry(
    entry: &crate::types::api::FileEntry,
    open_file: &send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    load_dir: &send_wrapper::SendWrapper<Fn1>,
    set_rename: WriteSignal<Option<ConfirmDeleteEntry>>,
    set_delete: WriteSignal<Option<ConfirmDeleteEntry>>,
    _delete_file: &send_wrapper::SendWrapper<Fn1>,
    _delete_dir: &send_wrapper::SendWrapper<Fn1>,
) -> impl IntoView {
    let path = entry.path.clone();
    let name = entry.name.clone();
    let is_dir = entry.is_dir;
    let size = entry.size;

    let ren_path = entry.path.clone();
    let ren_name = entry.name.clone();
    let dl_path = entry.path.clone();
    let del_path = entry.path.clone();
    let del_name = entry.name.clone();
    let click_path = entry.path.clone();
    let click_name = entry.name.clone();
    let of = open_file.clone();
    let ld = load_dir.clone();

    let swipe = use_swipe_reveal(SwipeConfig {
        actions_width: SWIPE_ACTIONS_WIDTH,
    });
    let on_ts = swipe.on_touch_start();
    let on_tm = swipe.on_touch_move();
    let on_te = swipe.on_touch_end();

    view! {
        <div
            class=move || format!("{} code-editor-file-entry-row", swipe.container_class())
            on:touchstart=move |ev| on_ts(ev)
            on:touchmove=move |ev| on_tm(ev)
            on:touchend=move |ev| on_te(ev)
        >
            <div class="swipe-row-actions">
                <button class="swipe-action-btn" title="Rename"
                    on:click=move |e: web_sys::MouseEvent| {
                        e.stop_propagation();
                        set_rename.set(Some(ConfirmDeleteEntry {
                            path: ren_path.clone(),
                            name: ren_name.clone(),
                            is_dir,
                        }));
                        swipe.close();
                    }
                ><IconPencil size=14 /></button>
                <button class="swipe-action-btn swipe-action-primary" title="Download"
                    on:click=move |e: web_sys::MouseEvent| {
                        e.stop_propagation();
                        let url = if is_dir {
                            crate::api::files::dir_download_url(&dl_path)
                        } else {
                            crate::api::files::file_download_url(&dl_path)
                        };
                        trigger_download(&url);
                        swipe.close();
                    }
                ><IconDownload size=14 /></button>
                <button class="swipe-action-btn swipe-action-danger" title="Delete"
                    on:click=move |e: web_sys::MouseEvent| {
                        e.stop_propagation();
                        set_delete.set(Some(ConfirmDeleteEntry {
                            path: del_path.clone(),
                            name: del_name.clone(),
                            is_dir,
                        }));
                        swipe.close();
                    }
                ><IconTrash2 size=14 /></button>
            </div>
            <div class="swipe-row-content" style=move || swipe.content_style()>
                <button
                    class="code-editor-file-entry"
                    on:click=move |_| {
                        if is_dir { ld(click_path.clone()); }
                        else { of(click_path.clone(), click_name.clone()); }
                    }
                >
                    {if is_dir {
                        view! { <IconFolder size=14 class="file-icon folder-icon" /> }.into_any()
                    } else {
                        view! { <IconFile size=14 class="file-icon" /> }.into_any()
                    }}
                    <span class="file-name">{name}</span>
                    {(!is_dir).then(|| {
                        view! { <span class="file-size">{format_size(size)}</span> }
                    })}
                </button>
            </div>
        </div>
    }
}
