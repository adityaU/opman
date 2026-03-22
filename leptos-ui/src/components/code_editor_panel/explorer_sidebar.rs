//! Explorer sidebar view — matches React DesktopLayout explorer panel.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::components::icons::{
    IconFile, IconFilePlus, IconFolderPlus, IconLoader2, IconPanelLeft, IconRefreshCw, IconUpload,
    IconX,
};
use crate::hooks::use_swipe_reveal::{use_swipe_reveal, SwipeConfig};

use super::actions::{Fn0, Fn1};
use super::explorer_ctx::{render_inline_create_input, ExplorerTreeCtx};
use super::explorer_tree::render_explorer_tree;
use super::state::EditorState;

/// Render the explorer sidebar (collapsible).
/// Returns `None` when collapsed.
pub fn render_explorer_sidebar(
    s: &EditorState,
    sidebar_width: ReadSignal<f64>,
    toggle_dir: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String)>>,
    open_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    handle_create_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    handle_create_dir: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String)>>,
    handle_delete_file: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String)>>,
    handle_delete_dir: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String)>>,
    handle_upload: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, web_sys::FileList)>>,
    close_file: send_wrapper::SendWrapper<Fn1>,
    handle_reload_dir: send_wrapper::SendWrapper<Fn1>,
    handle_reload_file: send_wrapper::SendWrapper<Fn1>,
    handle_reload_root: send_wrapper::SendWrapper<Fn0>,
    handle_rename: send_wrapper::SendWrapper<std::rc::Rc<dyn Fn(String, String, bool)>>,
) -> impl IntoView {
    let explorer_collapsed = s.explorer_collapsed;
    let set_explorer_collapsed = s.set_explorer_collapsed;
    let current_path = s.current_path;
    let open_files = s.open_files;
    let active_file = s.active_file;
    let set_active_file = s.set_active_file;
    let entries = s.entries;
    let explorer_loading = s.explorer_loading;
    let inline_create = s.inline_create;
    let set_inline_create = s.set_inline_create;
    let inline_confirm_delete = s.inline_confirm_delete;
    let set_inline_confirm_delete = s.set_inline_confirm_delete;
    let explorer_ctx_menu = s.explorer_ctx_menu;
    let set_explorer_ctx_menu = s.set_explorer_ctx_menu;
    let expanded_dirs = s.expanded_dirs;
    let dir_children = s.dir_children;
    let loading_dirs = s.loading_dirs;
    let file_action_busy = s.file_action_busy;
    let inline_rename = s.inline_rename;
    let set_inline_rename = s.set_inline_rename;

    let hcf_root = handle_create_file.clone();
    let hcd_root = handle_create_dir.clone();

    move || {
        if explorer_collapsed.get() {
            return None;
        }

        Some(view! {
            <div
                class="explorer-sidebar code-editor-explorer flex flex-col flex-shrink-0 border-r border-border-subtle"
                style:width=move || format!("{}px", sidebar_width.get())
            >
                // Header
                <div class="explorer-header">
                    <span class="explorer-title">"Explorer"</span>
                    <span class="explorer-header-actions">
                        <button class="explorer-hdr-btn" title="New file"
                            on:click=move |_| set_inline_create.set(Some((current_path.get_untracked(), "file".into())))
                        ><IconFilePlus size=13 /></button>
                        <button class="explorer-hdr-btn" title="New folder"
                            on:click=move |_| set_inline_create.set(Some((current_path.get_untracked(), "dir".into())))
                        ><IconFolderPlus size=13 /></button>
                        {move || file_action_busy.get().then(|| view! {
                            <span style="opacity:0.6;display:inline-flex;"><IconLoader2 size=13 class="spin" /></span>
                        })}
                        <button class="explorer-hdr-btn" title="Upload files"
                            on:click=move |_| {
                                if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                                    if let Some(el) = doc.get_element_by_id("explorer-upload-input") {
                                        let input: web_sys::HtmlInputElement = el.unchecked_into();
                                        input.click();
                                    }
                                }
                            }
                        ><IconUpload size=13 /></button>
                        <button class="explorer-hdr-btn" title="Reload explorer"
                            on:click={
                                let reload_root = handle_reload_root.clone();
                                move |_| reload_root()
                            }
                        ><IconRefreshCw size=13 /></button>
                        <button class="explorer-collapse-btn" title="Collapse explorer"
                            on:click=move |_| set_explorer_collapsed.set(true)
                        ><IconPanelLeft size=14 /></button>
                    </span>
                    <input id="explorer-upload-input" type="file" multiple=true style="display: none;"
                        on:change={
                            let handle_upload = handle_upload.clone();
                            move |e| {
                                let target: web_sys::HtmlInputElement = e.target().unwrap().unchecked_into();
                                if let Some(files) = target.files() {
                                    if files.length() > 0 { handle_upload(current_path.get_untracked(), files); }
                                }
                                target.set_value("");
                            }
                        }
                    />
                </div>

                // Open Files section
                {
                    let close_file = close_file.clone();
                    move || {
                        let files = open_files.get();
                        if files.is_empty() { return None; }
                        let active = active_file.get();
                        let close_file = close_file.clone();
                        Some(view! {
                            <div class="explorer-open-files">
                                <div class="explorer-section-label">"Open Files"</div>
                                {files.iter().map(|f| {
                                    let path = f.path.clone();
                                    let path_close = f.path.clone();
                                    let path_swipe_close = f.path.clone();
                                    let name = f.name.clone();
                                    let dirty = f.is_modified();
                                    let is_active = active.as_deref() == Some(&f.path);
                                    let close_file = close_file.clone();
                                    let close_file_swipe = close_file.clone();
                                    let swipe = use_swipe_reveal(SwipeConfig { actions_width: 52.0 });
                                    let on_ts = swipe.on_touch_start();
                                    let on_tm = swipe.on_touch_move();
                                    let on_te = swipe.on_touch_end();
                                    view! {
                                        <div
                                            class=move || {
                                                let base = if is_active { "explorer-open-file active" } else { "explorer-open-file" };
                                                format!("{} {base}", swipe.container_class())
                                            }
                                            on:click=move |_| set_active_file.set(Some(path.clone()))
                                            title=f.path.clone()
                                            on:touchstart=move |ev| on_ts(ev)
                                            on:touchmove=move |ev| on_tm(ev)
                                            on:touchend=move |ev| on_te(ev)
                                        >
                                            <div class="swipe-row-actions">
                                                <button class="swipe-action-btn swipe-action-danger" title="Close"
                                                    on:click=move |e: web_sys::MouseEvent| {
                                                        e.stop_propagation();
                                                        close_file_swipe(path_swipe_close.clone());
                                                        swipe.close();
                                                    }
                                                ><IconX size=14 /></button>
                                            </div>
                                            <div class="swipe-row-content" style=move || swipe.content_style()>
                                                <IconFile size=13 />
                                                <span class="file-name">{name}</span>
                                                {dirty.then(|| view! { <span class="open-file-modified-dot" /> })}
                                                <button class="open-file-close" on:click=move |e| {
                                                    e.stop_propagation();
                                                    close_file(path_close.clone());
                                                }><IconX size=12 /></button>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        })
                    }
                }

                // Root inline create
                {
                    let hcf = hcf_root.clone();
                    let hcd = hcd_root.clone();
                    move || {
                        let ic = inline_create.get();
                        match ic {
                            Some((ref parent, ref kind)) if parent == &current_path.get_untracked() || parent == "." => {
                                let kind_c = kind.clone();
                                let cancel = std::rc::Rc::new(move || set_inline_create.set(None));
                                let hcf = hcf.clone();
                                let hcd = hcd.clone();
                                let submit: std::rc::Rc<dyn Fn(String)> = {
                                    std::rc::Rc::new(move |name: String| {
                                        let ic2 = inline_create.get_untracked();
                                        if let Some((parent, kind)) = ic2 {
                                            if kind == "file" { hcf(parent, name); } else { hcd(parent, name); }
                                        }
                                        set_inline_create.set(None);
                                    })
                                };
                                Some(render_inline_create_input(&kind_c, 0, submit, cancel).into_any())
                            }
                            _ => None,
                        }
                    }
                }

                <div class="explorer-section-label">"Files"</div>

                <div class="explorer-tree">
                    {
                        let toggle_dir = toggle_dir.clone();
                        let open_file = open_file.clone();
                        let hcf = handle_create_file.clone();
                        let hcd = handle_create_dir.clone();
                        let hdf = handle_delete_file.clone();
                        let hdd = handle_delete_dir.clone();
                        let hrd = handle_reload_dir.clone();
                        let hrf = handle_reload_file.clone();
                        let hrn = handle_rename.clone();
                        move || {
                            if explorer_loading.get() {
                                return view! { <div class="code-editor-loading"><IconLoader2 size=16 class="spin" /></div> }.into_any();
                            }
                            let items = entries.get();
                            if items.is_empty() {
                                return view! { <div class="code-editor-empty">"Empty directory"</div> }.into_any();
                            }
                            let ctx = ExplorerTreeCtx {
                                expanded_dirs, dir_children, loading_dirs,
                                active_file_path: active_file,
                                inline_create, set_inline_create,
                                inline_confirm_delete, set_inline_confirm_delete,
                                inline_rename, set_inline_rename,
                                explorer_ctx_menu, set_explorer_ctx_menu,
                                toggle_dir: toggle_dir.clone(),
                                open_file: open_file.clone(),
                                handle_create_file: hcf.clone(),
                                handle_create_dir: hcd.clone(),
                                handle_delete_file: hdf.clone(),
                                handle_delete_dir: hdd.clone(),
                                handle_reload_dir: hrd.clone(),
                                handle_reload_file: hrf.clone(),
                                handle_rename: hrn.clone(),
                            };
                            render_explorer_tree(items, &ctx, 0).into_any()
                        }
                    }
                </div>
            </div>
        })
    }
}
