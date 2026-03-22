//! Recursive explorer tree rendering — matches React `ExplorerTree.tsx`.
//!
//! File nodes include swipe-to-reveal actions for mobile (rename,
//! download, delete). Directory nodes live in `explorer_dir_node.rs`.

use leptos::prelude::*;

use crate::components::icons::{IconDownload, IconFile, IconPencil, IconRefreshCw, IconTrash2};
use crate::hooks::use_swipe_reveal::{use_swipe_reveal, SwipeConfig};
use crate::types::api::FileEntry;

use super::explorer_ctx::{
    render_confirm_delete_inline, render_inline_rename_input, ExplorerTreeCtx,
};
use super::explorer_dir_node::render_dir_node;
use super::types::ConfirmDeleteEntry;

/// Width of 3 swipe action buttons (34px each) + gaps + tray padding.
const SWIPE_ACTIONS_WIDTH: f64 = 128.0;

/// Render a list of entries recursively at the given `depth`.
pub fn render_explorer_tree(
    entries: Vec<FileEntry>,
    ctx: &ExplorerTreeCtx,
    depth: u32,
) -> impl IntoView {
    entries
        .into_iter()
        .map(|entry| {
            if entry.is_dir {
                render_dir_node(entry, depth, ctx).into_any()
            } else {
                render_file_node(entry, depth, ctx).into_any()
            }
        })
        .collect_view()
}

/// Render a single file node with hover actions (desktop) and
/// swipe-to-reveal actions (mobile).
fn render_file_node(entry: FileEntry, depth: u32, ctx: &ExplorerTreeCtx) -> impl IntoView {
    let ctx = ctx.clone();
    let entry_path = entry.path.clone();
    let entry_name = entry.name.clone();
    let padding = format!("{}px", 8 + depth * 14 + 14);

    let ep_enter = entry_path.clone();
    let ep_leave = entry_path.clone();
    let ep_active = entry_path.clone();
    let ep_click = entry_path.clone();
    let ep_ctx = entry_path.clone();
    let ep_reload = entry_path.clone();
    let ep_download = entry_path.clone();
    let ep_del = entry_path.clone();
    let ep_ren = entry_path.clone();
    let ren_name = entry_name.clone();
    let ep_confirm = entry_path.clone();
    let ep_rename_check = entry_path.clone();

    // Swipe clones
    let ep_swipe_ren = entry_path.clone();
    let ren_swipe_name = entry_name.clone();
    let ep_swipe_dl = entry_path.clone();
    let ep_swipe_del = entry_path.clone();

    let set_ctx_menu = ctx.set_explorer_ctx_menu;
    let ctx_menu = ctx.explorer_ctx_menu;
    let set_icd = ctx.set_inline_confirm_delete;
    let set_ir = ctx.set_inline_rename;
    let open_file = ctx.open_file.clone();
    let hdf = ctx.handle_delete_file.clone();
    let hrf = ctx.handle_reload_file.clone();
    let hrn = ctx.handle_rename.clone();
    let name_click = entry_name.clone();

    // Swipe state
    let swipe = use_swipe_reveal(SwipeConfig {
        actions_width: SWIPE_ACTIONS_WIDTH,
    });
    let on_ts = swipe.on_touch_start();
    let on_tm = swipe.on_touch_move();
    let on_te = swipe.on_touch_end();

    view! {
        <>
            <div
                class=move || format!("{} explorer-tree-entry-row", swipe.container_class())
                on:mouseenter=move |_| set_ctx_menu.set(Some(ep_enter.clone()))
                on:mouseleave=move |_| {
                    if ctx_menu.get_untracked().as_deref() == Some(&ep_leave) {
                        set_ctx_menu.set(None);
                    }
                }
                on:touchstart=move |ev| on_ts(ev)
                on:touchmove=move |ev| on_tm(ev)
                on:touchend=move |ev| on_te(ev)
            >
                // Swipe action tray (behind content)
                <div class="swipe-row-actions">
                    <button class="swipe-action-btn" title="Rename"
                        on:click=move |e: web_sys::MouseEvent| {
                            e.stop_propagation();
                            set_ir.set(Some(ConfirmDeleteEntry {
                                path: ep_swipe_ren.clone(),
                                name: ren_swipe_name.clone(),
                                is_dir: false,
                            }));
                            set_ctx_menu.set(None);
                            swipe.close();
                        }
                    ><IconPencil size=14 /></button>
                    <button class="swipe-action-btn swipe-action-primary" title="Download"
                        on:click=move |e: web_sys::MouseEvent| {
                            e.stop_propagation();
                            let url = crate::api::files::file_download_url(&ep_swipe_dl);
                            trigger_download(&url);
                            swipe.close();
                        }
                    ><IconDownload size=14 /></button>
                    <button class="swipe-action-btn swipe-action-danger" title="Delete"
                        on:click=move |e: web_sys::MouseEvent| {
                            e.stop_propagation();
                            set_icd.set(Some(ConfirmDeleteEntry {
                                path: ep_swipe_del.clone(),
                                name: ep_swipe_del.rsplit('/').next().unwrap_or(&ep_swipe_del).to_string(),
                                is_dir: false,
                            }));
                            set_ctx_menu.set(None);
                            swipe.close();
                        }
                    ><IconTrash2 size=14 /></button>
                </div>

                // Front content layer (slides left on swipe)
                <div class="swipe-row-content" style=move || swipe.content_style()>
                    <button
                        class=move || {
                            if ctx.active_file_path.get().as_deref() == Some(&*ep_active) {
                                "explorer-tree-entry explorer-tree-file active"
                            } else {
                                "explorer-tree-entry explorer-tree-file"
                            }
                        }
                        style:padding-left=padding.clone()
                        on:click={
                            let open_file = open_file.clone();
                            let name_click = name_click.clone();
                            move |_| open_file(ep_click.clone(), name_click.clone())
                        }
                    >
                        <IconFile size=14 class="file-icon" />
                        <span class="file-name">{entry_name}</span>
                    </button>
                    // Hover action buttons (desktop)
                    {move || {
                        if ctx_menu.get().as_deref() != Some(&*ep_ctx) {
                            return None;
                        }
                        let epdel = ep_del.clone();
                        let epr = ep_reload.clone();
                        let epf_dl = ep_download.clone();
                        let ep_rn = ep_ren.clone();
                        let rn_nm = ren_name.clone();
                        let hrf2 = hrf.clone();
                        Some(view! {
                            <span class="explorer-entry-actions">
                                <button class="explorer-action-btn" title="Rename"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        set_ir.set(Some(ConfirmDeleteEntry {
                                            path: ep_rn.clone(),
                                            name: rn_nm.clone(),
                                            is_dir: false,
                                        }));
                                        set_ctx_menu.set(None);
                                    }
                                ><IconPencil size=12 /></button>
                                <button class="explorer-action-btn" title="Reload file"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        hrf2(epr.clone());
                                    }
                                ><IconRefreshCw size=12 /></button>
                                <button class="explorer-action-btn" title="Download file"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        let url = crate::api::files::file_download_url(&epf_dl);
                                        trigger_download(&url);
                                    }
                                ><IconDownload size=12 /></button>
                                <button class="explorer-action-btn explorer-action-danger" title="Delete file"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        set_icd.set(Some(ConfirmDeleteEntry {
                                            path: epdel.clone(),
                                            name: epdel.rsplit('/').next().unwrap_or(&epdel).to_string(),
                                            is_dir: false,
                                        }));
                                        set_ctx_menu.set(None);
                                    }
                                ><IconTrash2 size=12 /></button>
                            </span>
                        })
                    }}
                </div>
            </div>
            // Inline rename overlay for this file
            {
                let hrn2 = hrn.clone();
                let ep_r = ep_rename_check.clone();
                move || {
                    let ir = ctx.inline_rename.get();
                    if ir.as_ref().map(|r| r.path.as_str()) != Some(&*ep_r) {
                        return None;
                    }
                    let entry = ir.unwrap();
                    let hrn3 = hrn2.clone();
                    let p = ep_r.clone();
                    let cancel = std::rc::Rc::new(move || set_ir.set(None));
                    let submit: std::rc::Rc<dyn Fn(String)> = std::rc::Rc::new(move |new_name: String| {
                        hrn3(p.clone(), new_name, false);
                        set_ir.set(None);
                    });
                    Some(render_inline_rename_input(&entry.name, false, depth, submit, cancel))
                }
            }
            {
                let hdf2 = hdf.clone();
                let ep_c = ep_confirm.clone();
                move || {
                    let cd = ctx.inline_confirm_delete.get();
                    if cd.as_ref().map(|c| c.path.as_str()) != Some(&*ep_c) {
                        return None;
                    }
                    let hdf3 = hdf2.clone();
                    let p = ep_c.clone();
                    let p2 = ep_c.clone();
                    let cancel = std::rc::Rc::new(move || set_icd.set(None));
                    let confirm = std::rc::Rc::new({ let hdf3 = hdf3.clone(); move || hdf3(p.clone()) });
                    Some(render_confirm_delete_inline(&p2, false, depth, confirm, cancel))
                }
            }
        </>
    }
}

/// Trigger a browser download by creating a temporary `<a>` element.
pub fn trigger_download(url: &str) {
    use wasm_bindgen::JsCast;
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
    if let Some(body) = doc.body() {
        body.append_child(&a).ok();
        a.click();
        body.remove_child(&a).ok();
    }
}
