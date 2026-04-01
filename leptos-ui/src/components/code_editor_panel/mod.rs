//! CodeEditorPanel — file explorer + tabbed editor + preview renderers.
//! Matches React `CodeEditorPanel.tsx` — switches between DesktopLayout
//! (side-by-side explorer + editor) and MobileLayout (flat file browser
//! or full-screen editor) at 768 px breakpoint.
//!
//! Both layouts are always rendered; we toggle visibility via CSS so that
//! `impl IntoView` values are consumed exactly once.

mod actions;
mod actions_ext;
mod actions_reload;
mod doc_renderers;
mod document_editor;
mod editor_body;
mod editor_toolbar;
mod explorer_ctx;
mod explorer_sidebar;
mod explorer_dir_node;
mod explorer_tree;
mod file_renderers;
mod js_helpers;
mod mobile_layout;
mod mobile_overlays;
mod mobile_toolbar;
pub mod native_editor;
mod spreadsheet_editor;
mod state;
mod types;

use leptos::prelude::*;

use crate::components::icons::{IconPanelLeft, IconX};
use crate::hooks::use_panel_state::PanelState;
use crate::hooks::use_resizable::{use_resizable, ResizableOptions, ResizeDirection};
use crate::components::debug_overlay::dbg_log;

use actions::{
    build_close_file, build_create, build_load_dir, build_load_dir_children,
    build_open_file, build_refresh_subtree, build_reload_dir, build_reload_file,
    build_reload_root, build_revert_file, build_save_file, build_toggle_dir,
};
use actions_ext::{
    build_definition, build_delete_dir, build_delete_file, build_format,
    build_hover, build_rename_entry, build_set_active_view, build_upload,
    install_save_shortcut,
};
use editor_body::render_editor_body;
use editor_toolbar::render_editor_toolbar;
use explorer_sidebar::render_explorer_sidebar;
use mobile_layout::render_mobile_browser;
use state::EditorState;

#[component]
pub fn CodeEditorPanel(panels: PanelState) -> impl IntoView {
    dbg_log("[EDITOR-PANEL] CodeEditorPanel constructor called");
    let s = EditorState::new();
    let explorer_resize = use_resizable(ResizableOptions {
        initial_size: 200.0,
        min_size: 140.0,
        max_size: 300.0,
        direction: ResizeDirection::Horizontal,
        reverse: false,
    });
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
    let upload = build_upload(&s, refresh.clone());
    let rename_entry = build_rename_entry(&s, refresh);
    let hover = build_hover(&s);
    let definition = build_definition(&s, open_file.clone());
    let format = build_format(&s);
    let set_active_view = build_set_active_view(&s);
    let reload_dir = build_reload_dir(&s);
    let reload_file = build_reload_file(&s);
    let reload_root = build_reload_root(&s);

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

    // ── Editor SSE: auto-reload files changed on disk ───────────────
    {
        let open_files_sig = s.open_files;
        let set_open_files = s.set_open_files;
        let active_file_sig = s.active_file;
        let set_save_status = s.set_save_status;
        Effect::new(move |_prev: Option<()>| {
            let Ok(es) = crate::sse::connection::create_editor_events_sse() else {
                log::error!("[EDITOR-SSE] Failed to create editor events SSE");
                return;
            };
            let cb = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::MessageEvent)>::new(
                move |e: web_sys::MessageEvent| {
                    let data = e.data().as_string().unwrap_or_default();
                    #[derive(serde::Deserialize)]
                    struct FileChanged {
                        path: String,
                        source: String,
                    }
                    let Ok(evt) = serde_json::from_str::<FileChanged>(&data) else {
                        return;
                    };
                    // Only auto-reload if the file is open and NOT locally modified.
                    let files = open_files_sig.get_untracked();
                    let Some(file) = files.iter().find(|f| f.path == evt.path) else {
                        return; // File not open — ignore.
                    };
                    if file.is_modified() && evt.source != "web_save" {
                        dbg_log(&format!(
                            "[EDITOR-SSE] {} changed externally but has local edits — skipping reload",
                            evt.path
                        ));
                        return;
                    }
                    // Re-read the file from disk and update the open file entry.
                    let path = evt.path.clone();
                    let is_doc = types::is_doc_render_type(&file.render_type);
                    leptos::task::spawn_local(async move {
                        if is_doc {
                            match crate::api::files::doc_read(&path).await {
                                Ok(r) => {
                                    set_open_files.update(|fs| {
                                        if let Some(f) = fs.iter_mut().find(|f| f.path == path) {
                                            f.doc_data = Some(r.data);
                                            f.edited_doc_data = None;
                                        }
                                    });
                                    if active_file_sig.get_untracked().as_deref() == Some(&path) {
                                        set_save_status.set(None);
                                    }
                                }
                                Err(e) => log::error!("[EDITOR-SSE] doc_read {path}: {e}"),
                            }
                        } else {
                            match crate::api::files::file_read(&path).await {
                                Ok(r) => {
                                    set_open_files.update(|fs| {
                                        if let Some(f) = fs.iter_mut().find(|f| f.path == path) {
                                            f.content = r.content;
                                            f.language = r.language;
                                            f.edited_content = None;
                                        }
                                    });
                                    if active_file_sig.get_untracked().as_deref() == Some(&path) {
                                        set_save_status.set(None);
                                    }
                                }
                                Err(e) => log::error!("[EDITOR-SSE] reload {path}: {e}"),
                            }
                        }
                    });
                },
            );
            use wasm_bindgen::JsCast;
            let _ = es.add_event_listener_with_callback(
                "file_changed",
                cb.as_ref().unchecked_ref(),
            );
            cb.forget();
            // Clean up on scope disposal
            leptos::prelude::on_cleanup(move || {
                es.close();
            });
        });
    }

    // ── Mobile: back-to-browser handler ─────────────────────────────
    let set_active_file_back = s.set_active_file;
    let set_save_status_back = s.set_save_status;
    let on_back: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(move || {
        set_active_file_back.set(None);
        set_save_status_back.set(None);
    });

    // ── Back-navigation integration for file-open state ───────────────
    // When a file is opened (mobile or desktop), push a custom back layer
    // so the browser back button returns to the file list/explorer instead
    // of closing the entire editor panel.
    {
        let active_file = s.active_file;
        let set_active_file = s.set_active_file;
        let set_save_status = s.set_save_status;
        let back_nav =
            leptos::prelude::use_context::<crate::hooks::use_back_navigation::BackNavigation>();

        if let Some(back_nav) = back_nav {
            Effect::new(move |prev_has_file: Option<bool>| {
                let has_file = active_file.get().is_some();

                if let Some(had_file) = prev_has_file {
                    if has_file && !had_file {
                        let set_af = set_active_file;
                        let set_ss = set_save_status;
                        back_nav.push_custom_layer(
                            "editor_file_open",
                            std::rc::Rc::new(move || {
                                set_af.set(None);
                                set_ss.set(None);
                            }),
                        );
                    } else if !has_file && had_file {
                        back_nav.remove_custom_layer("editor_file_open");
                    }
                }

                has_file
            });
        }
    }

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
        send_wrapper::SendWrapper::new(reload_root.clone()),
        send_wrapper::SendWrapper::new(rename_entry.clone()),
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
        explorer_resize.size,
        send_wrapper::SendWrapper::new(toggle_dir),
        send_wrapper::SendWrapper::new(open_file),
        send_wrapper::SendWrapper::new(create_file),
        send_wrapper::SendWrapper::new(create_dir),
        send_wrapper::SendWrapper::new(delete_file),
        send_wrapper::SendWrapper::new(delete_dir),
        send_wrapper::SendWrapper::new(upload),
        send_wrapper::SendWrapper::new(close_file.clone()),
        send_wrapper::SendWrapper::new(reload_dir),
        send_wrapper::SendWrapper::new(reload_file),
        send_wrapper::SendWrapper::new(reload_root),
        send_wrapper::SendWrapper::new(rename_entry),
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
            // Explorer resize handle
            <div
                class=move || {
                    let base = "resize-handle resize-handle-horizontal w-1 cursor-col-resize bg-transparent hover:bg-border-active transition-colors flex-shrink-0";
                    if explorer_resize.is_dragging.get() {
                        format!("{base} dragging bg-border-active")
                    } else {
                        base.to_string()
                    }
                }
                style:display=move || if explorer_collapsed.get() { "none" } else { "" }
                on:mousedown=move |e| explorer_resize.start_drag(e)
                on:touchstart=move |e| explorer_resize.start_drag_touch(e)
            />
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
                        view! {
                            <For
                                each=move || open_files.get()
                                key=|f| f.path.clone()
                                children=move |f: types::OpenFile| {
                                    let path = f.path.clone();
                                    let path_click = f.path.clone();
                                    let path_close = f.path.clone();
                                    let name = f.name.clone();
                                    let modified = f.is_modified();
                                    let close_file = close_file.clone();
                                    view! {
                                        <div
                                            class=move || {
                                                let base = "code-editor-tab flex items-center gap-1 px-2.5 py-1 text-xs rounded-t transition-colors group cursor-pointer max-w-[160px]";
                                                if active_file.get().as_deref() == Some(path.as_str()) {
                                                    format!("{base} active bg-bg-panel text-text border-b-2 border-primary")
                                                } else {
                                                    format!("{base} text-text-muted hover:text-text hover:bg-bg-panel/50")
                                                }
                                            }
                                            on:click=move |_| set_active_file_tab.set(Some(path_click.clone()))
                                        >
                                            {modified.then(|| view! { <span class="code-editor-modified-dot">"•"</span> })}
                                            <span class="truncate">{name}</span>
                                            <button
                                                class="opacity-0 group-hover:opacity-100 text-text-muted hover:text-error text-[10px] ml-0.5"
                                                on:click=move |e| { e.stop_propagation(); close_file(path_close.clone()); }
                                            ><IconX size=12 /></button>
                                        </div>
                                    }
                                }
                            />
                        }
                    }
                </div>
                {d_toolbar}
                {d_body}
            </div>
        </div>
    }
}
