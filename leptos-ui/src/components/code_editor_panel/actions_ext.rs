//! Additional action builders — delete, upload, LSP, keyboard shortcut, view mode.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::state::EditorState;
use super::types::EditorViewMode;

use super::actions::Fn1;

pub fn build_delete_file(s: &EditorState, close: Fn1, refresh: Fn1) -> Fn1 {
    let open_files = s.open_files;
    let set_busy = s.set_file_action_busy;
    std::rc::Rc::new(move |fp: String| {
        set_busy.set(true);
        let close = close.clone();
        let refresh = refresh.clone();
        leptos::task::spawn_local(async move {
            match crate::api::files::file_delete(&fp).await {
                Ok(()) => {
                    if open_files.get_untracked().iter().any(|f| f.path == fp) {
                        close(fp.clone());
                    }
                    let parent = if fp.contains('/') {
                        fp[..fp.rfind('/').unwrap()].to_string()
                    } else { ".".into() };
                    refresh(parent);
                }
                Err(e) => log::error!("delete file: {e}"),
            }
            set_busy.set(false);
        });
    })
}

pub fn build_delete_dir(s: &EditorState, close: Fn1, refresh: Fn1) -> Fn1 {
    let open_files = s.open_files;
    let set_expanded = s.set_expanded_dirs;
    let set_busy = s.set_file_action_busy;
    std::rc::Rc::new(move |dp: String| {
        set_busy.set(true);
        let close = close.clone();
        let refresh = refresh.clone();
        leptos::task::spawn_local(async move {
            match crate::api::files::dir_delete(&dp).await {
                Ok(()) => {
                    let to_close: Vec<String> = open_files.get_untracked().iter()
                        .filter(|f| f.path.starts_with(&format!("{dp}/")) || f.path == dp)
                        .map(|f| f.path.clone()).collect();
                    for p in to_close { close(p); }
                    set_expanded.update(|s| { s.remove(&dp); });
                    let parent = if dp.contains('/') {
                        dp[..dp.rfind('/').unwrap()].to_string()
                    } else { ".".into() };
                    refresh(parent);
                }
                Err(e) => log::error!("delete dir: {e}"),
            }
            set_busy.set(false);
        });
    })
}

pub fn build_upload(s: &EditorState, refresh: Fn1) -> std::rc::Rc<dyn Fn(String, web_sys::FileList)> {
    let set_busy = s.set_file_action_busy;
    std::rc::Rc::new(move |dir: String, files: web_sys::FileList| {
        set_busy.set(true);
        let refresh = refresh.clone();
        let d = dir.clone();
        leptos::task::spawn_local(async move {
            match crate::api::files::file_upload(&d, &files).await {
                Ok(_) => refresh(d),
                Err(e) => log::error!("upload: {e}"),
            }
            set_busy.set(false);
        });
    })
}

pub fn build_hover(s: &EditorState) -> std::rc::Rc<dyn Fn()> {
    let af = s.active_file;
    let cl = s.cursor_line;
    let cc = s.cursor_col;
    let set_busy = s.set_lsp_busy;
    let set_avail = s.set_lsp_available;
    let set_hover = s.set_hover_text;
    std::rc::Rc::new(move || {
        let Some(path) = af.get_untracked() else { return; };
        let (line, col) = (cl.get_untracked(), cc.get_untracked());
        set_busy.set(Some("hover".into()));
        leptos::task::spawn_local(async move {
            match crate::api::editor::lsp_hover(&path, line, col).await {
                Ok(r) => {
                    set_avail.set(true);
                    set_hover.set(r.content.or(Some("No hover information available at cursor.".into())));
                }
                Err(_) => set_hover.set(Some("Hover information unavailable.".into())),
            }
            set_busy.set(None);
        });
    })
}

pub fn build_definition(
    s: &EditorState,
    open_file: std::rc::Rc<dyn Fn(String, String)>,
) -> std::rc::Rc<dyn Fn()> {
    let af = s.active_file;
    let cl = s.cursor_line;
    let cc = s.cursor_col;
    let set_busy = s.set_lsp_busy;
    let set_avail = s.set_lsp_available;
    let set_jump = s.set_pending_jump_line;
    std::rc::Rc::new(move || {
        let Some(path) = af.get_untracked() else { return; };
        let (line, col) = (cl.get_untracked(), cc.get_untracked());
        let open = open_file.clone();
        set_busy.set(Some("definition".into()));
        leptos::task::spawn_local(async move {
            match crate::api::editor::lsp_definition(&path, line, col).await {
                Ok(r) => {
                    set_avail.set(true);
                    if let Some(loc) = r.location {
                        if loc.lnum > 0 { set_jump.set(Some(loc.lnum)); }
                        let name = loc.file.rsplit('/').next().unwrap_or(&loc.file).to_string();
                        open(loc.file, name);
                    }
                }
                Err(_) => log::warn!("Definition lookup unavailable"),
            }
            set_busy.set(None);
        });
    })
}

pub fn build_format(s: &EditorState) -> std::rc::Rc<dyn Fn()> {
    let af = s.active_file;
    let of = s.open_files;
    let set_of = s.set_open_files;
    let set_busy = s.set_lsp_busy;
    let set_avail = s.set_lsp_available;
    let set_status = s.set_save_status;
    std::rc::Rc::new(move || {
        let Some(path) = af.get_untracked() else { return; };
        let content = of.get_untracked().iter()
            .find(|f| f.path == path).map(|f| f.current_content().to_string());
        let Some(content) = content else { return; };
        set_busy.set(Some("format".into()));
        leptos::task::spawn_local(async move {
            match crate::api::editor::lsp_format(&path, &content).await {
                Ok(r) => {
                    set_avail.set(true);
                    set_of.update(|fs| {
                        if let Some(f) = fs.iter_mut().find(|f| f.path == path) {
                            f.content = r.formatted.clone();
                            f.edited_content = None;
                        }
                    });
                    set_status.set(Some("saved".into()));
                    leptos::task::spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(1500).await;
                        set_status.set(None);
                    });
                }
                Err(_) => log::warn!("LSP format unavailable"),
            }
            set_busy.set(None);
        });
    })
}

pub fn build_set_active_view(s: &EditorState) -> std::rc::Rc<dyn Fn(EditorViewMode)> {
    let af = s.active_file;
    let set_vm = s.set_view_modes;
    std::rc::Rc::new(move |mode: EditorViewMode| {
        if let Some(path) = af.get_untracked() {
            set_vm.update(|m| { m.insert(path, mode); });
        }
    })
}

/// Install the Ctrl/Cmd+S keyboard shortcut. Call once in the component.
pub fn install_save_shortcut(active_file: ReadSignal<Option<String>>, save: Fn1) {
    let handler = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(
        move |e: web_sys::KeyboardEvent| {
            if (e.meta_key() || e.ctrl_key()) && e.key() == "s" {
                e.prevent_default();
                if let Some(p) = active_file.get_untracked() { save(p); }
            }
        },
    );
    let js_fn: js_sys::Function = handler.as_ref().unchecked_ref::<js_sys::Function>().clone();
    let window = web_sys::window().unwrap();
    let _ = window.add_event_listener_with_callback("keydown", &js_fn);
    handler.forget();
    on_cleanup(move || {
        if let Some(w) = web_sys::window() {
            let _ = w.remove_event_listener_with_callback("keydown", &js_fn);
        }
    });
}
