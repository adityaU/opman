//! Action closures for the code editor panel (file ops, explorer, LSP).
//! Each builder returns `Rc<dyn Fn(...)>` for cheap cloning in view closures.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::state::EditorState;
use super::types::{classify_file, is_binary_render_type, EditorViewMode, OpenFile};

pub type Fn1 = std::rc::Rc<dyn Fn(String)>;
pub type Fn2 = std::rc::Rc<dyn Fn(String, String)>;
pub type Fn0 = std::rc::Rc<dyn Fn()>;

pub fn build_load_dir(s: &EditorState) -> Fn1 {
    let set_entries = s.set_entries;
    let set_current_path = s.set_current_path;
    let set_loading = s.set_explorer_loading;
    std::rc::Rc::new(move |path: String| {
        set_loading.set(true);
        leptos::task::spawn_local(async move {
            match crate::api::files::file_browse(&path).await {
                Ok(r) => { set_entries.set(r.entries); set_current_path.set(r.path); }
                Err(e) => log::error!("browse: {e}"),
            }
            set_loading.set(false);
        });
    })
}

pub fn build_load_dir_children(s: &EditorState) -> Fn1 {
    let set_dc = s.set_dir_children;
    let set_ld = s.set_loading_dirs;
    std::rc::Rc::new(move |dir: String| {
        set_ld.update(|s| { s.insert(dir.clone()); });
        let dp = dir.clone();
        leptos::task::spawn_local(async move {
            match crate::api::files::file_browse(&dp).await {
                Ok(r) => { set_dc.update(|m| { m.insert(dp.clone(), r.entries); }); }
                Err(e) => log::error!("expand dir: {e}"),
            }
            set_ld.update(|s| { s.remove(&dp); });
        });
    })
}

pub fn build_close_file(s: &EditorState) -> Fn1 {
    let open_files = s.open_files;
    let set_open_files = s.set_open_files;
    let active_file = s.active_file;
    let set_active_file = s.set_active_file;
    std::rc::Rc::new(move |path: String| {
        set_open_files.update(|files| {
            let idx = files.iter().position(|f| f.path == path);
            files.retain(|f| f.path != path);
            if active_file.get_untracked().as_deref() == Some(&path) {
                if let Some(i) = idx {
                    let ni = i.min(files.len().saturating_sub(1));
                    set_active_file.set(files.get(ni).map(|f| f.path.clone()));
                } else {
                    set_active_file.set(None);
                }
            }
        });
    })
}

pub fn build_refresh_subtree(s: &EditorState) -> Fn1 {
    let current_path = s.current_path;
    let expanded_dirs = s.expanded_dirs;
    let set_entries = s.set_entries;
    let set_current_path = s.set_current_path;
    let set_dc = s.set_dir_children;
    std::rc::Rc::new(move |dir_path: String| {
        let cur = current_path.get_untracked();
        let expanded = expanded_dirs.get_untracked();
        leptos::task::spawn_local(async move {
            match crate::api::files::file_browse(&cur).await {
                Ok(r) => {
                    set_entries.set(r.entries);
                    set_current_path.set(r.path);
                }
                Err(e) => log::error!("refresh browse({cur:?}): {e}"),
            }
            if dir_path != "." && expanded.contains(&dir_path) {
                if let Ok(r) = crate::api::files::file_browse(&dir_path).await {
                    set_dc.update(|m| { m.insert(dir_path.clone(), r.entries); });
                }
            }
            let parent = if dir_path.contains('/') {
                dir_path[..dir_path.rfind('/').unwrap()].to_string()
            } else { ".".to_string() };
            if parent != dir_path && expanded.contains(&parent) {
                if let Ok(r) = crate::api::files::file_browse(&parent).await {
                    set_dc.update(|m| { m.insert(parent, r.entries); });
                }
            }
        });
    })
}

pub fn build_open_file(s: &EditorState) -> Fn2 {
    let open_files = s.open_files;
    let set_open_files = s.set_open_files;
    let set_active = s.set_active_file;
    let set_loading = s.set_editor_loading;
    let set_vm = s.set_view_modes;
    let set_diag = s.set_diagnostics;
    std::rc::Rc::new(move |path: String, name: String| {
        if open_files.get_untracked().iter().any(|f| f.path == path) {
            set_active.set(Some(path));
            return;
        }
        let rt = classify_file(&path);
        set_vm.update(|m| { m.entry(path.clone()).or_insert(EditorViewMode::Code); });
        if is_binary_render_type(&rt) {
            let f = OpenFile { path: path.clone(), name, language: String::new(),
                content: String::new(), edited_content: None, render_type: rt };
            set_open_files.update(|fs| fs.push(f));
            set_active.set(Some(path));
            return;
        }
        set_loading.set(true);
        let pc = path.clone();
        leptos::task::spawn_local(async move {
            match crate::api::files::file_read(&pc).await {
                Ok(r) => {
                    let rt = classify_file(&r.path);
                    let f = OpenFile { path: r.path.clone(), name, language: r.language.clone(),
                        content: r.content.clone(), edited_content: None, render_type: rt };
                    set_open_files.update(|fs| fs.push(f));
                    set_active.set(Some(r.path));
                    let dp = pc.clone();
                    leptos::task::spawn_local(async move {
                        if let Ok(r) = crate::api::editor::lsp_diagnostics(&dp).await {
                            set_diag.set(r.diagnostics);
                        }
                    });
                }
                Err(e) => log::error!("read file: {e}"),
            }
            set_loading.set(false);
        });
    })
}

pub fn build_save_file(s: &EditorState) -> Fn1 {
    let open_files = s.open_files;
    let set_open_files = s.set_open_files;
    let set_saving = s.set_saving;
    let set_status = s.set_save_status;
    std::rc::Rc::new(move |path: String| {
        let content = open_files.get_untracked().iter()
            .find(|f| f.path == path).map(|f| f.current_content().to_string());
        let Some(content) = content else { return; };
        set_saving.set(true);
        set_status.set(Some("saving".into()));
        let pc = path.clone();
        leptos::task::spawn_local(async move {
            match crate::api::files::file_write(&pc, &content).await {
                Ok(_) => {
                    set_open_files.update(|fs| {
                        if let Some(f) = fs.iter_mut().find(|f| f.path == pc) {
                            f.content = content.clone(); f.edited_content = None;
                        }
                    });
                    set_status.set(Some("saved".into()));
                    set_saving.set(false);
                    leptos::task::spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(2000).await;
                        set_status.set(None);
                    });
                }
                Err(e) => { log::error!("save: {e}"); set_status.set(None); set_saving.set(false); }
            }
        });
    })
}

pub fn build_revert_file(s: &EditorState) -> Fn1 {
    let set_open_files = s.set_open_files;
    std::rc::Rc::new(move |path: String| {
        set_open_files.update(|fs| {
            if let Some(f) = fs.iter_mut().find(|f| f.path == path) {
                f.edited_content = None;
            }
        });
    })
}

pub fn build_toggle_dir(s: &EditorState, load_children: Fn1) -> Fn1 {
    let expanded = s.expanded_dirs;
    let set_expanded = s.set_expanded_dirs;
    let dc = s.dir_children;
    std::rc::Rc::new(move |path: String| {
        let mut exp = expanded.get_untracked();
        if exp.contains(&path) {
            exp.remove(&path);
        } else {
            exp.insert(path.clone());
            if !dc.get_untracked().contains_key(&path) { load_children(path); }
        }
        set_expanded.set(exp);
    })
}

pub fn build_create(s: &EditorState, refresh: Fn1, is_file: bool) -> Fn2 {
    let set_busy = s.set_file_action_busy;
    std::rc::Rc::new(move |parent: String, name: String| {
        let fp = if parent == "." { name.clone() } else { format!("{parent}/{name}") };
        set_busy.set(true);
        let pd = parent.clone();
        let refresh = refresh.clone();
        leptos::task::spawn_local(async move {
            let r = if is_file {
                crate::api::files::file_create(&fp).await
            } else {
                crate::api::files::dir_create(&fp).await
            };
            match r { Ok(()) => refresh(pd), Err(e) => log::error!("create: {e}") }
            set_busy.set(false);
        });
    })
}

/// Reload a single directory listing. Updates root entries if the dir is current,
/// and updates dir_children if the dir is expanded.
pub fn build_reload_dir(s: &EditorState) -> Fn1 {
    let current_path = s.current_path;
    let expanded_dirs = s.expanded_dirs;
    let set_entries = s.set_entries;
    let set_current_path = s.set_current_path;
    let set_dc = s.set_dir_children;
    std::rc::Rc::new(move |dir_path: String| {
        let cur = current_path.get_untracked();
        let expanded = expanded_dirs.get_untracked();
        let browse_path = if dir_path == "." { cur.clone() } else { dir_path.clone() };
        leptos::task::spawn_local(async move {
            match crate::api::files::file_browse(&browse_path).await {
                Ok(r) => {
                    if dir_path == "." || dir_path == cur {
                        set_entries.set(r.entries.clone());
                        set_current_path.set(r.path);
                    }
                    if dir_path != "." && expanded.contains(&dir_path) {
                        set_dc.update(|m| { m.insert(dir_path, r.entries); });
                    }
                }
                Err(e) => log::error!("reload dir: {e}"),
            }
        });
    })
}

/// Reload an open file from disk — re-reads content, clears edited state.
/// Skips binary-like render types.
pub fn build_reload_file(s: &EditorState) -> Fn1 {
    let open_files = s.open_files;
    let set_open_files = s.set_open_files;
    let active_file = s.active_file;
    let set_save_status = s.set_save_status;
    std::rc::Rc::new(move |file_path: String| {
        let files = open_files.get_untracked();
        let existing = files.iter().find(|f| f.path == file_path);
        if existing.is_none() { return; }
        let rt = classify_file(&file_path);
        if is_binary_render_type(&rt) { return; }
        let fp = file_path.clone();
        leptos::task::spawn_local(async move {
            match crate::api::files::file_read(&fp).await {
                Ok(r) => {
                    set_open_files.update(|fs| {
                        if let Some(f) = fs.iter_mut().find(|f| f.path == fp) {
                            f.content = r.content;
                            f.language = r.language;
                            f.edited_content = None;
                        }
                    });
                    if active_file.get_untracked().as_deref() == Some(&fp) {
                        set_save_status.set(None);
                    }
                }
                Err(e) => log::error!("reload file: {e}"),
            }
        });
    })
}

/// Reload root: re-fetch current directory listing + all expanded directories.
pub fn build_reload_root(s: &EditorState) -> Fn0 {
    let current_path = s.current_path;
    let expanded_dirs = s.expanded_dirs;
    let set_entries = s.set_entries;
    let set_current_path = s.set_current_path;
    let set_dc = s.set_dir_children;
    std::rc::Rc::new(move || {
        let cur = current_path.get_untracked();
        let expanded: Vec<String> = expanded_dirs.get_untracked().into_iter().collect();
        leptos::task::spawn_local(async move {
            if let Ok(r) = crate::api::files::file_browse(&cur).await {
                set_entries.set(r.entries);
                set_current_path.set(r.path);
            }
            for dir in expanded {
                if let Ok(r) = crate::api::files::file_browse(&dir).await {
                    let d = dir.clone();
                    set_dc.update(|m| { m.insert(d, r.entries); });
                }
            }
        });
    })
}
