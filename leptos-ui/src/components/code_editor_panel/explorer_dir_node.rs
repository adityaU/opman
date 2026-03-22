//! Explorer directory node with swipe-to-reveal actions on mobile.

use leptos::prelude::*;

use crate::components::icons::{
    IconChevronDown, IconChevronRight, IconDownload, IconFilePlus, IconFolder, IconFolderPlus,
    IconLoader2, IconPencil, IconRefreshCw, IconTrash2,
};
use crate::hooks::use_swipe_reveal::{use_swipe_reveal, SwipeConfig};
use crate::types::api::FileEntry;

use super::explorer_ctx::{
    render_confirm_delete_inline, render_inline_create_input, render_inline_rename_input,
    ExplorerTreeCtx,
};
use super::explorer_tree::{render_explorer_tree, trigger_download};
use super::types::ConfirmDeleteEntry;

/// Width of 3 swipe action buttons (34px each) + gaps + tray padding.
const SWIPE_ACTIONS_WIDTH: f64 = 128.0;

/// Render a single directory node with hover + swipe actions.
pub fn render_dir_node(entry: FileEntry, depth: u32, ctx: &ExplorerTreeCtx) -> impl IntoView {
    let ctx = ctx.clone();
    let entry_path = entry.path.clone();
    let entry_name = entry.name.clone();
    let padding = format!("{}px", 8 + depth * 14);

    let ep_enter = entry_path.clone();
    let ep_leave = entry_path.clone();
    let ep_toggle = entry_path.clone();
    let ep_chevron = entry_path.clone();
    let ep_ctx = entry_path.clone();
    let ep_reload = entry_path.clone();
    let ep_download = entry_path.clone();
    let ep_new_file = entry_path.clone();
    let ep_new_dir = entry_path.clone();
    let ep_del = entry_path.clone();
    let ep_ren = entry_path.clone();
    let ren_name = entry_name.clone();
    let ep_confirm = entry_path.clone();
    let ep_rename_check = entry_path.clone();
    let ep_ic = entry_path.clone();
    let ep_children = entry_path.clone();
    // Swipe clones
    let ep_swipe_ren = entry_path.clone();
    let ren_swipe_name = entry_name.clone();
    let ep_swipe_new = entry_path.clone();
    let ep_swipe_del = entry_path.clone();

    let toggle = ctx.toggle_dir.clone();
    let set_ctx_menu = ctx.set_explorer_ctx_menu;
    let ctx_menu = ctx.explorer_ctx_menu;
    let set_ic = ctx.set_inline_create;
    let set_icd = ctx.set_inline_confirm_delete;
    let set_ir = ctx.set_inline_rename;
    let hcf = ctx.handle_create_file.clone();
    let hcd = ctx.handle_create_dir.clone();
    let hdd = ctx.handle_delete_dir.clone();
    let hrd = ctx.handle_reload_dir.clone();
    let hrn = ctx.handle_rename.clone();
    // Swipe state
    let swipe = use_swipe_reveal(SwipeConfig {
        actions_width: SWIPE_ACTIONS_WIDTH,
    });
    let on_ts = swipe.on_touch_start();
    let on_tm = swipe.on_touch_move();
    let on_te = swipe.on_touch_end();
    view! {
        <div>
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
                                is_dir: true,
                            }));
                            set_ctx_menu.set(None);
                            swipe.close();
                        }
                    ><IconPencil size=14 /></button>
                    <button class="swipe-action-btn swipe-action-primary" title="New file"
                        on:click=move |e: web_sys::MouseEvent| {
                            e.stop_propagation();
                            set_ic.set(Some((ep_swipe_new.clone(), "file".to_string())));
                            set_ctx_menu.set(None);
                            swipe.close();
                        }
                    ><IconFilePlus size=14 /></button>
                    <button class="swipe-action-btn swipe-action-danger" title="Delete"
                        on:click=move |e: web_sys::MouseEvent| {
                            e.stop_propagation();
                            set_icd.set(Some(ConfirmDeleteEntry {
                                path: ep_swipe_del.clone(),
                                name: ep_swipe_del.rsplit('/').next().unwrap_or(&ep_swipe_del).to_string(),
                                is_dir: true,
                            }));
                            set_ctx_menu.set(None);
                            swipe.close();
                        }
                    ><IconTrash2 size=14 /></button>
                </div>

                // Front content layer (slides left on swipe)
                <div class="swipe-row-content" style=move || swipe.content_style()>
                    <button
                        class="explorer-tree-entry explorer-tree-dir"
                        style:padding-left=padding.clone()
                        on:click={
                            let toggle = toggle.clone();
                            move |_| toggle(ep_toggle.clone())
                        }
                    >
                        {
                            let loading = ctx.loading_dirs;
                            let expanded = ctx.expanded_dirs;
                            move || {
                                let path = ep_chevron.clone();
                                if loading.get().contains(&path) {
                                    view! { <IconLoader2 size=12 class="spin explorer-tree-chevron" /> }.into_any()
                                } else if expanded.get().contains(&path) {
                                    view! { <IconChevronDown size=12 class="explorer-tree-chevron" /> }.into_any()
                                } else {
                                    view! { <IconChevronRight size=12 class="explorer-tree-chevron" /> }.into_any()
                                }
                            }
                        }
                        <IconFolder size=14 class="file-icon folder-icon" />
                        <span class="file-name">{entry_name}</span>
                    </button>
                    // Hover action buttons (desktop)
                    {move || {
                        if ctx_menu.get().as_deref() != Some(&ep_ctx) {
                            return None;
                        }
                        let epf = ep_new_file.clone();
                        let epd = ep_new_dir.clone();
                        let epdel = ep_del.clone();
                        let epr = ep_reload.clone();
                        let epd_dl = ep_download.clone();
                        let ep_rn = ep_ren.clone();
                        let rn_nm = ren_name.clone();
                        let hrd2 = hrd.clone();
                        Some(view! {
                            <span class="explorer-entry-actions">
                                <button class="explorer-action-btn" title="Rename"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        set_ir.set(Some(ConfirmDeleteEntry {
                                            path: ep_rn.clone(),
                                            name: rn_nm.clone(),
                                            is_dir: true,
                                        }));
                                        set_ctx_menu.set(None);
                                    }
                                ><IconPencil size=12 /></button>
                                <button class="explorer-action-btn" title="Reload folder"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        hrd2(epr.clone());
                                    }
                                ><IconRefreshCw size=12 /></button>
                                <button class="explorer-action-btn" title="Download folder as zip"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        let url = crate::api::files::dir_download_url(&epd_dl);
                                        trigger_download(&url);
                                    }
                                ><IconDownload size=12 /></button>
                                <button class="explorer-action-btn" title="New file"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        set_ic.set(Some((epf.clone(), "file".to_string())));
                                        set_ctx_menu.set(None);
                                    }
                                ><IconFilePlus size=12 /></button>
                                <button class="explorer-action-btn" title="New folder"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        set_ic.set(Some((epd.clone(), "dir".to_string())));
                                        set_ctx_menu.set(None);
                                    }
                                ><IconFolderPlus size=12 /></button>
                                <button class="explorer-action-btn explorer-action-danger" title="Delete folder"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        set_icd.set(Some(ConfirmDeleteEntry {
                                            path: epdel.clone(),
                                            name: epdel.rsplit('/').next().unwrap_or(&epdel).to_string(),
                                            is_dir: true,
                                        }));
                                        set_ctx_menu.set(None);
                                    }
                                ><IconTrash2 size=12 /></button>
                            </span>
                        })
                    }}
                </div>
            </div>
            // Inline rename overlay for this dir
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
                        hrn3(p.clone(), new_name, true);
                        set_ir.set(None);
                    });
                    Some(render_inline_rename_input(&entry.name, true, depth, submit, cancel))
                }
            }
            // Confirm delete overlay for this dir
            {
                let hdd2 = hdd.clone();
                let ep_c = ep_confirm.clone();
                move || {
                    let cd = ctx.inline_confirm_delete.get();
                    if cd.as_ref().map(|c| c.path.as_str()) != Some(&*ep_c) {
                        return None;
                    }
                    let hdd3 = hdd2.clone();
                    let p = ep_c.clone();
                    let p2 = ep_c.clone();
                    let cancel = std::rc::Rc::new(move || set_icd.set(None));
                    let confirm = std::rc::Rc::new({ let hdd3 = hdd3.clone(); move || hdd3(p.clone()) });
                    Some(render_confirm_delete_inline(&p2, true, depth, confirm, cancel))
                }
            }
            // Inline create input for this dir
            {
                let hcf2 = hcf.clone();
                let hcd2 = hcd.clone();
                let ep_i = ep_ic.clone();
                move || {
                    let ic = ctx.inline_create.get();
                    if ic.as_ref().map(|(p, _)| p.as_str()) != Some(&*ep_i) {
                        return None;
                    }
                    let kind = ic.unwrap().1;
                    let hcf3 = hcf2.clone();
                    let hcd3 = hcd2.clone();
                    let ep_s = ep_i.clone();
                    let kind2 = kind.clone();
                    let cancel = std::rc::Rc::new(move || set_ic.set(None));
                    let ic_signal = ctx.inline_create;
                    let submit: std::rc::Rc<dyn Fn(String)> = std::rc::Rc::new(move |name: String| {
                        if ic_signal.get_untracked().is_none() { return; }
                        if kind2 == "file" { hcf3(ep_s.clone(), name); } else { hcd3(ep_s.clone(), name); }
                        set_ic.set(None);
                    });
                    Some(render_inline_create_input(&kind, depth + 1, submit, cancel))
                }
            }
            // Children (recursive)
            {
                let ctx2 = ctx.clone();
                move || {
                    let expanded = ctx2.expanded_dirs.get();
                    let children_map = ctx2.dir_children.get();
                    let path = ep_children.clone();
                    if !expanded.contains(&path) {
                        return None;
                    }
                    let children = children_map.get(&path).cloned().unwrap_or_default();
                    if children.is_empty() {
                        return None;
                    }
                    Some(view! {
                        <div class="explorer-tree-children">
                            {render_explorer_tree(children, &ctx2, depth + 1)}
                        </div>
                    })
                }
            }
        </div>
    }
}
