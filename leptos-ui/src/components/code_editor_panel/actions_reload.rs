//! Reload-specific action builders for the code editor panel.
//! Separated from `actions.rs` to stay within the 300-line limit.

use leptos::prelude::*;

use super::state::EditorState;
use super::types::{classify_file, is_binary_render_type, is_doc_render_type};

use super::actions::{Fn0, Fn1};

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
/// Skips binary-like render types. Uses doc-read for document types.
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
        if is_doc_render_type(&rt) {
            leptos::task::spawn_local(async move {
                match crate::api::files::doc_read(&fp).await {
                    Ok(r) => {
                        set_open_files.update(|fs| {
                            if let Some(f) = fs.iter_mut().find(|f| f.path == fp) {
                                f.doc_data = Some(r.data);
                                f.edited_doc_data = None;
                            }
                        });
                        if active_file.get_untracked().as_deref() == Some(&fp) {
                            set_save_status.set(None);
                        }
                    }
                    Err(e) => log::error!("reload doc: {e}"),
                }
            });
            return;
        }
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
