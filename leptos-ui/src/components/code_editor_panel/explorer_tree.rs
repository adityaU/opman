//! Recursive explorer tree rendering — matches React `ExplorerTree.tsx`.

use leptos::prelude::*;

use crate::components::icons::{
    IconChevronDown, IconChevronRight, IconFile, IconFilePlus, IconFolder, IconFolderPlus,
    IconLoader2, IconTrash2,
};
use crate::types::api::FileEntry;

use super::explorer_ctx::{
    render_confirm_delete_inline, render_inline_create_input, ExplorerTreeCtx,
};
use super::types::ConfirmDeleteEntry;

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

/// Render a single directory node in the explorer tree.
fn render_dir_node(entry: FileEntry, depth: u32, ctx: &ExplorerTreeCtx) -> impl IntoView {
    let ctx = ctx.clone();
    let entry_path = entry.path.clone();
    let entry_name = entry.name.clone();
    let padding = format!("{}px", 8 + depth * 14);

    let ep_enter = entry_path.clone();
    let ep_leave = entry_path.clone();
    let ep_toggle = entry_path.clone();
    let ep_chevron = entry_path.clone();
    let ep_ctx = entry_path.clone();
    let ep_new_file = entry_path.clone();
    let ep_new_dir = entry_path.clone();
    let ep_del = entry_path.clone();
    let ep_confirm = entry_path.clone();
    let ep_ic = entry_path.clone();
    let ep_children = entry_path.clone();

    let toggle = ctx.toggle_dir.clone();
    let set_ctx_menu = ctx.set_explorer_ctx_menu;
    let ctx_menu = ctx.explorer_ctx_menu;
    let set_ic = ctx.set_inline_create;
    let set_icd = ctx.set_inline_confirm_delete;
    let hcf = ctx.handle_create_file.clone();
    let hcd = ctx.handle_create_dir.clone();
    let hdd = ctx.handle_delete_dir.clone();

    view! {
        <div>
            <div
                class="explorer-tree-entry-row"
                on:mouseenter=move |_| set_ctx_menu.set(Some(ep_enter.clone()))
                on:mouseleave=move |_| {
                    if ctx_menu.get_untracked().as_deref() == Some(&ep_leave) {
                        set_ctx_menu.set(None);
                    }
                }
            >
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
                // Hover action buttons
                {move || {
                    if ctx_menu.get().as_deref() != Some(&ep_ctx) {
                        return None;
                    }
                    let epf = ep_new_file.clone();
                    let epd = ep_new_dir.clone();
                    let epdel = ep_del.clone();
                    Some(view! {
                        <span class="explorer-entry-actions">
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
                    let submit: std::rc::Rc<dyn Fn(String)> = std::rc::Rc::new(move |name: String| {
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

/// Render a single file node in the explorer tree.
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
    let ep_del = entry_path.clone();
    let ep_confirm = entry_path.clone();

    let set_ctx_menu = ctx.set_explorer_ctx_menu;
    let ctx_menu = ctx.explorer_ctx_menu;
    let set_icd = ctx.set_inline_confirm_delete;
    let open_file = ctx.open_file.clone();
    let hdf = ctx.handle_delete_file.clone();
    let name_click = entry_name.clone();

    view! {
        <>
            <div
                class="explorer-tree-entry-row"
                on:mouseenter=move |_| set_ctx_menu.set(Some(ep_enter.clone()))
                on:mouseleave=move |_| {
                    if ctx_menu.get_untracked().as_deref() == Some(&ep_leave) {
                        set_ctx_menu.set(None);
                    }
                }
            >
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
                {move || {
                    if ctx_menu.get().as_deref() != Some(&*ep_ctx) {
                        return None;
                    }
                    let epdel = ep_del.clone();
                    Some(view! {
                        <span class="explorer-entry-actions">
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
