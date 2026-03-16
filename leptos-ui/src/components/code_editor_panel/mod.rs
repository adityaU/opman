//! CodeEditorPanel — file explorer + tabbed editor + preview renderers.
//! Matches React `CodeEditorPanel.tsx` — switches between DesktopLayout
//! (side-by-side explorer + editor) and MobileLayout (flat file browser
//! or full-screen editor) at 768 px breakpoint.
//!
//! Both layouts are always rendered; we toggle visibility via CSS so that
//! `impl IntoView` values are consumed exactly once.

mod actions;
mod actions_ext;
mod editor_body;
mod editor_toolbar;
mod explorer_ctx;
mod explorer_sidebar;
mod explorer_tree;
mod file_renderers;
mod js_helpers;
mod mobile_layout;
pub mod native_editor;
mod state;
mod types;

use leptos::prelude::*;

use crate::components::icons::{IconPanelLeft, IconX};
use crate::hooks::use_panel_state::PanelState;

use actions::{
    build_close_file, build_create, build_load_dir, build_load_dir_children,
    build_open_file, build_refresh_subtree, build_revert_file, build_save_file,
    build_toggle_dir,
};
use actions_ext::{
    build_definition, build_delete_dir, build_delete_file, build_format,
    build_hover, build_set_active_view, build_upload, install_save_shortcut,
};
use editor_body::render_editor_body;
use editor_toolbar::render_editor_toolbar;
use explorer_sidebar::render_explorer_sidebar;
use mobile_layout::render_mobile_browser;
use state::EditorState;

#[component]
pub fn CodeEditorPanel(panels: PanelState) -> impl IntoView {
    let s = EditorState::new();
    let active_view = s.active_view_memo();
    let breadcrumbs = s.breadcrumbs_memo();
    let is_mobile = s.is_mobile;

    // Build all action closures
    let load_dir = build_load_dir(&s);
    let load_dir_children = build_load_dir_children(&s);
    let close_file = build_close_file(&s);
    let refresh = build_refresh_subtree(&s);
    let open_file = build_open_file(&s);
    let save_file = build_save_file(&s);
    let revert_file = build_revert_file(&s);
    let toggle_dir = build_toggle_dir(&s, load_dir_children);
    let create_file = build_create(&s, refresh.clone(), true);
    let create_dir = build_create(&s, refresh.clone(), false);
    let delete_file = build_delete_file(&s, close_file.clone(), refresh.clone());
    let delete_dir = build_delete_dir(&s, close_file.clone(), refresh.clone());
    let upload = build_upload(&s, refresh);
    let hover = build_hover(&s);
    let definition = build_definition(&s, open_file.clone());
    let format = build_format(&s);
    let set_active_view = build_set_active_view(&s);

    // Initial load
    let load_dir_init = load_dir.clone();
    {
        let load = load_dir_init.clone();
        Effect::new(move |prev: Option<()>| {
            if prev.is_none() { load(".".to_string()); }
        });
    }

    // Per-project state save/restore
    {
        let project_ctx = leptos::prelude::use_context::<crate::hooks::use_project_context::ProjectContext>();
        if let Some(ctx) = project_ctx {
            let s_inner = s.clone();
            let load = load_dir_init;
            Effect::new(move |prev_idx: Option<usize>| {
                let new_idx = ctx.index.get();
                if let Some(old) = prev_idx {
                    if old != new_idx {
                        // Save current project's editor state
                        s_inner.save_for_project(old);
                        // Restore new project's state or reset + reload
                        if !s_inner.restore_for_project(new_idx) {
                            s_inner.reset_to_defaults();
                            load(".".to_string());
                        }
                    }
                }
                new_idx
            });
        }
    }

    // Keyboard shortcut
    install_save_shortcut(s.active_file, save_file.clone());

    // ── Mobile: back-to-browser handler ─────────────────────────────
    let set_active_file_back = s.set_active_file;
    let set_save_status_back = s.set_save_status;
    let on_back: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(move || {
        set_active_file_back.set(None);
        set_save_status_back.set(None);
    });

    // ── Build mobile views (consumed once) ──────────────────────────
    let mobile_browser = render_mobile_browser(
        &s, breadcrumbs,
        send_wrapper::SendWrapper::new(load_dir.clone()),
        send_wrapper::SendWrapper::new(open_file.clone()),
        send_wrapper::SendWrapper::new(create_file.clone()),
        send_wrapper::SendWrapper::new(create_dir.clone()),
        send_wrapper::SendWrapper::new(delete_file.clone()),
        send_wrapper::SendWrapper::new(delete_dir.clone()),
        send_wrapper::SendWrapper::new(upload.clone()),
    );
    let m_toolbar = render_editor_toolbar(
        &s, active_view,
        save_file.clone(), revert_file.clone(),
        hover.clone(), definition.clone(), format.clone(),
        set_active_view.clone(),
        is_mobile, Some(on_back),
    );
    let m_body = render_editor_body(&s, active_view);

    // ── Build desktop views (consumed once) ─────────────────────────
    let d_sidebar = render_explorer_sidebar(
        &s,
        send_wrapper::SendWrapper::new(toggle_dir),
        send_wrapper::SendWrapper::new(open_file),
        send_wrapper::SendWrapper::new(create_file),
        send_wrapper::SendWrapper::new(create_dir),
        send_wrapper::SendWrapper::new(delete_file),
        send_wrapper::SendWrapper::new(delete_dir),
        send_wrapper::SendWrapper::new(upload),
        send_wrapper::SendWrapper::new(close_file.clone()),
    );
    let d_toolbar = render_editor_toolbar(
        &s, active_view, save_file, revert_file,
        hover, definition, format, set_active_view,
        is_mobile, None,
    );
    let d_body = render_editor_body(&s, active_view);

    // Desktop tab state
    let open_files = s.open_files;
    let active_file = s.active_file;
    let set_active_file_tab = s.set_active_file;
    let explorer_collapsed = s.explorer_collapsed;
    let set_explorer_collapsed = s.set_explorer_collapsed;
    let close_file_tabs = send_wrapper::SendWrapper::new(close_file);

    // Both layouts always rendered; visibility toggled via style.
    view! {
        // ── Mobile layout ───────────────────────────────────────────
        <div style:display=move || if is_mobile.get() { "" } else { "none" }>
            // Mobile: browser when no file open
            <div style:display=move || if active_file.get().is_none() { "" } else { "none" }>
                {mobile_browser}
            </div>
            // Mobile: editor when file is open
            <div class="code-editor-panel"
                 style:display=move || if active_file.get().is_some() { "" } else { "none" }>
                {m_toolbar}
                {m_body}
            </div>
        </div>
        // ── Desktop layout ──────────────────────────────────────────
        <div class="code-editor-panel code-editor-desktop flex h-full bg-bg-panel"
             style:display=move || if is_mobile.get() { "none" } else { "" }>
            {d_sidebar}
            <div class="editor-area flex flex-col flex-1 min-w-0">
                {move || explorer_collapsed.get().then(|| view! {
                    <button
                        class="explorer-expand-btn flex items-center justify-center w-6 h-6 m-1 text-text-muted hover:text-text rounded hover:bg-bg-element"
                        title="Show explorer"
                        on:click=move |_| set_explorer_collapsed.set(false)
                    ><IconPanelLeft size=14 /></button>
                })}
                <div class="editor-tabs code-editor-tabs flex items-center gap-0.5 px-1 py-0.5 bg-bg-element border-b border-border-subtle overflow-x-auto">
                    {
                        let close_file = close_file_tabs.clone();
                        move || {
                            open_files.get().iter().map(|f| {
                                let path = f.path.clone();
                                let path_close = f.path.clone();
                                let name = f.name.clone();
                                let modified = f.is_modified();
                                let is_active = active_file.get().as_deref() == Some(&f.path);
                                let close_file = close_file.clone();
                                view! {
                                    <div
                                        class=move || {
                                            let base = "code-editor-tab flex items-center gap-1 px-2.5 py-1 text-xs rounded-t transition-colors group cursor-pointer max-w-[160px]";
                                            if is_active {
                                                format!("{base} active bg-bg-panel text-text border-b-2 border-primary")
                                            } else {
                                                format!("{base} text-text-muted hover:text-text hover:bg-bg-panel/50")
                                            }
                                        }
                                        on:click=move |_| set_active_file_tab.set(Some(path.clone()))
                                    >
                                        {modified.then(|| view! { <span class="code-editor-modified-dot">"•"</span> })}
                                        <span class="truncate">{name}</span>
                                        <button
                                            class="opacity-0 group-hover:opacity-100 text-text-muted hover:text-error text-[10px] ml-0.5"
                                            on:click=move |e| { e.stop_propagation(); close_file(path_close.clone()); }
                                        ><IconX size=12 /></button>
                                    </div>
                                }
                            }).collect::<Vec<_>>()
                        }
                    }
                </div>
                {d_toolbar}
                {d_body}
            </div>
        </div>
    }
}
