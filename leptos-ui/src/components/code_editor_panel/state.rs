//! Editor state signals — all reactive signals used by CodeEditorPanel.
//! Includes per-project snapshot cache for save/restore on project switch.

use leptos::prelude::*;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::types::{ConfirmDeleteEntry, EditorViewMode, OpenFile};

/// A breadcrumb segment for mobile directory navigation.
#[derive(Clone, Debug, PartialEq)]
pub struct BreadcrumbEntry {
    pub path: String,
    pub label: String,
}

/// Snapshot of editor state for per-project caching.
#[derive(Clone, Debug)]
struct EditorSnapshot {
    current_path: String,
    entries: Vec<crate::types::api::FileEntry>,
    expanded_dirs: HashSet<String>,
    dir_children: HashMap<String, Vec<crate::types::api::FileEntry>>,
    explorer_collapsed: bool,
    open_files: Vec<OpenFile>,
    active_file: Option<String>,
    view_modes: HashMap<String, EditorViewMode>,
}

/// All reactive state for the CodeEditorPanel component.
#[derive(Clone)]
pub struct EditorState {
    // Explorer
    pub current_path: ReadSignal<String>,
    pub set_current_path: WriteSignal<String>,
    pub entries: ReadSignal<Vec<crate::types::api::FileEntry>>,
    pub set_entries: WriteSignal<Vec<crate::types::api::FileEntry>>,
    pub explorer_loading: ReadSignal<bool>,
    pub set_explorer_loading: WriteSignal<bool>,
    pub expanded_dirs: ReadSignal<std::collections::HashSet<String>>,
    pub set_expanded_dirs: WriteSignal<std::collections::HashSet<String>>,
    pub dir_children:
        ReadSignal<std::collections::HashMap<String, Vec<crate::types::api::FileEntry>>>,
    pub set_dir_children:
        WriteSignal<std::collections::HashMap<String, Vec<crate::types::api::FileEntry>>>,
    pub loading_dirs: ReadSignal<std::collections::HashSet<String>>,
    pub set_loading_dirs: WriteSignal<std::collections::HashSet<String>>,
    pub explorer_collapsed: ReadSignal<bool>,
    pub set_explorer_collapsed: WriteSignal<bool>,
    pub inline_create: ReadSignal<Option<(String, String)>>,
    pub set_inline_create: WriteSignal<Option<(String, String)>>,
    pub inline_confirm_delete: ReadSignal<Option<ConfirmDeleteEntry>>,
    pub set_inline_confirm_delete: WriteSignal<Option<ConfirmDeleteEntry>>,
    pub explorer_ctx_menu: ReadSignal<Option<String>>,
    pub set_explorer_ctx_menu: WriteSignal<Option<String>>,
    pub file_action_busy: ReadSignal<bool>,
    pub set_file_action_busy: WriteSignal<bool>,

    // Editor / tabs
    pub open_files: ReadSignal<Vec<OpenFile>>,
    pub set_open_files: WriteSignal<Vec<OpenFile>>,
    pub active_file: ReadSignal<Option<String>>,
    pub set_active_file: WriteSignal<Option<String>>,
    pub editor_loading: ReadSignal<bool>,
    pub set_editor_loading: WriteSignal<bool>,
    pub view_modes: ReadSignal<std::collections::HashMap<String, EditorViewMode>>,
    pub set_view_modes: WriteSignal<std::collections::HashMap<String, EditorViewMode>>,

    // Save
    pub save_status: ReadSignal<Option<String>>,
    pub set_save_status: WriteSignal<Option<String>>,
    pub saving: ReadSignal<bool>,
    pub set_saving: WriteSignal<bool>,

    // LSP
    pub diagnostics: ReadSignal<Vec<crate::types::api::EditorLspDiagnostic>>,
    pub set_diagnostics: WriteSignal<Vec<crate::types::api::EditorLspDiagnostic>>,
    pub lsp_available: ReadSignal<bool>,
    pub set_lsp_available: WriteSignal<bool>,
    pub lsp_busy: ReadSignal<Option<String>>,
    pub set_lsp_busy: WriteSignal<Option<String>>,
    pub hover_text: ReadSignal<Option<String>>,
    pub set_hover_text: WriteSignal<Option<String>>,

    // Cursor
    pub cursor_line: ReadSignal<u32>,
    pub set_cursor_line: WriteSignal<u32>,
    pub cursor_col: ReadSignal<u32>,
    pub set_cursor_col: WriteSignal<u32>,

    // Jump to line (e.g. from go-to-definition)
    pub pending_jump_line: ReadSignal<Option<u32>>,
    pub set_pending_jump_line: WriteSignal<Option<u32>>,

    // Mermaid
    pub mermaid_svg: ReadSignal<String>,
    pub set_mermaid_svg: WriteSignal<String>,
    pub mermaid_error: ReadSignal<Option<String>>,
    pub set_mermaid_error: WriteSignal<Option<String>>,

    // Mobile
    pub is_mobile: ReadSignal<bool>,
    pub set_is_mobile: WriteSignal<bool>,
    /// Mobile: actions dropdown visibility
    pub mobile_actions_open: ReadSignal<bool>,
    pub set_mobile_actions_open: WriteSignal<bool>,
    /// Mobile: inline create state ("file" or "dir")
    pub mobile_inline_create: ReadSignal<Option<String>>,
    pub set_mobile_inline_create: WriteSignal<Option<String>>,
    /// Mobile: confirm delete state
    pub mobile_confirm_delete: ReadSignal<Option<ConfirmDeleteEntry>>,
    pub set_mobile_confirm_delete: WriteSignal<Option<ConfirmDeleteEntry>>,

    // Per-project snapshot cache
    project_snapshots: StoredValue<HashMap<usize, EditorSnapshot>>,
}

impl EditorState {
    /// Create all signals. Call once in the component body.
    pub fn new() -> Self {
        let (current_path, set_current_path) = signal(String::from("."));
        let (entries, set_entries) = signal(Vec::new());
        let (explorer_loading, set_explorer_loading) = signal(false);
        let (expanded_dirs, set_expanded_dirs) = signal(std::collections::HashSet::new());
        let (dir_children, set_dir_children) = signal(std::collections::HashMap::new());
        let (loading_dirs, set_loading_dirs) = signal(std::collections::HashSet::new());
        let (explorer_collapsed, set_explorer_collapsed) = signal(false);
        let (inline_create, set_inline_create) = signal(None);
        let (inline_confirm_delete, set_inline_confirm_delete) = signal(None);
        let (explorer_ctx_menu, set_explorer_ctx_menu) = signal(None);
        let (file_action_busy, set_file_action_busy) = signal(false);

        let (open_files, set_open_files) = signal(Vec::new());
        let (active_file, set_active_file) = signal(None);
        let (editor_loading, set_editor_loading) = signal(false);
        let (view_modes, set_view_modes) = signal(std::collections::HashMap::new());

        let (save_status, set_save_status) = signal(None);
        let (saving, set_saving) = signal(false);

        let (diagnostics, set_diagnostics) = signal(Vec::new());
        let (lsp_available, set_lsp_available) = signal(false);
        let (lsp_busy, set_lsp_busy) = signal(None);
        let (hover_text, set_hover_text) = signal(None);

        let (cursor_line, set_cursor_line) = signal(1u32);
        let (cursor_col, set_cursor_col) = signal(1u32);

        let (pending_jump_line, set_pending_jump_line) = signal(None);

        let (mermaid_svg, set_mermaid_svg) = signal(String::new());
        let (mermaid_error, set_mermaid_error) = signal(None);

        // Mobile detection via matchMedia (768px breakpoint, matching React)
        let initial_mobile = web_sys::window()
            .map(|w| {
                w.inner_width()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1024.0)
                    < 768.0
            })
            .unwrap_or(false);
        let (is_mobile, set_is_mobile) = signal(initial_mobile);
        if let Some(win) = web_sys::window() {
            if let Ok(Some(mq)) = win.match_media("(min-width: 768px)") {
                let cb = Closure::<dyn Fn(web_sys::Event)>::new(move |e: web_sys::Event| {
                    // The event target is the MediaQueryList itself
                    if let Some(mq) = e
                        .target()
                        .and_then(|t| t.dyn_into::<web_sys::MediaQueryList>().ok())
                    {
                        set_is_mobile.set(!mq.matches());
                    }
                });
                let _ = mq.add_event_listener_with_callback("change", cb.as_ref().unchecked_ref());
                cb.forget(); // lives for page lifetime
            }
        }

        let (mobile_actions_open, set_mobile_actions_open) = signal(false);
        let (mobile_inline_create, set_mobile_inline_create) = signal(None);
        let (mobile_confirm_delete, set_mobile_confirm_delete) = signal(None);

        let project_snapshots = StoredValue::new(HashMap::<usize, EditorSnapshot>::new());

        Self {
            current_path,
            set_current_path,
            entries,
            set_entries,
            explorer_loading,
            set_explorer_loading,
            expanded_dirs,
            set_expanded_dirs,
            dir_children,
            set_dir_children,
            loading_dirs,
            set_loading_dirs,
            explorer_collapsed,
            set_explorer_collapsed,
            inline_create,
            set_inline_create,
            inline_confirm_delete,
            set_inline_confirm_delete,
            explorer_ctx_menu,
            set_explorer_ctx_menu,
            file_action_busy,
            set_file_action_busy,
            open_files,
            set_open_files,
            active_file,
            set_active_file,
            editor_loading,
            set_editor_loading,
            view_modes,
            set_view_modes,
            save_status,
            set_save_status,
            saving,
            set_saving,
            diagnostics,
            set_diagnostics,
            lsp_available,
            set_lsp_available,
            lsp_busy,
            set_lsp_busy,
            hover_text,
            set_hover_text,
            cursor_line,
            set_cursor_line,
            cursor_col,
            set_cursor_col,
            pending_jump_line,
            set_pending_jump_line,
            mermaid_svg,
            set_mermaid_svg,
            mermaid_error,
            set_mermaid_error,
            is_mobile,
            set_is_mobile,
            mobile_actions_open,
            set_mobile_actions_open,
            mobile_inline_create,
            set_mobile_inline_create,
            mobile_confirm_delete,
            set_mobile_confirm_delete,
            project_snapshots,
        }
    }

    /// Derived: active view mode for the current file.
    pub fn active_view_memo(&self) -> Memo<EditorViewMode> {
        let active_file = self.active_file;
        let view_modes = self.view_modes;
        Memo::new(move |_| {
            let af = active_file.get();
            let modes = view_modes.get();
            af.and_then(|p| modes.get(&p).cloned())
                .unwrap_or(EditorViewMode::Code)
        })
    }

    /// Derived: breadcrumbs from current_path (for mobile navigation).
    pub fn breadcrumbs_memo(&self) -> Memo<Vec<BreadcrumbEntry>> {
        let current_path = self.current_path;
        Memo::new(move |_| {
            let cp = current_path.get();
            let mut crumbs = vec![BreadcrumbEntry {
                path: ".".into(),
                label: "root".into(),
            }];
            if cp != "." {
                let parts: Vec<&str> = cp.split('/').collect();
                for (i, part) in parts.iter().enumerate() {
                    crumbs.push(BreadcrumbEntry {
                        path: parts[..=i].join("/"),
                        label: part.to_string(),
                    });
                }
            }
            crumbs
        })
    }

    /// Save current editor state for the given project index.
    pub fn save_for_project(&self, project_idx: usize) {
        let snap = EditorSnapshot {
            current_path: self.current_path.get_untracked(),
            entries: self.entries.get_untracked(),
            expanded_dirs: self.expanded_dirs.get_untracked(),
            dir_children: self.dir_children.get_untracked(),
            explorer_collapsed: self.explorer_collapsed.get_untracked(),
            open_files: self.open_files.get_untracked(),
            active_file: self.active_file.get_untracked(),
            view_modes: self.view_modes.get_untracked(),
        };
        self.project_snapshots.update_value(|map| {
            map.insert(project_idx, snap);
        });
    }

    /// Restore editor state for the given project index.
    /// Returns true if a snapshot was found; false means caller should do initial load.
    pub fn restore_for_project(&self, project_idx: usize) -> bool {
        let snap = self
            .project_snapshots
            .with_value(|map| map.get(&project_idx).cloned());
        if let Some(s) = snap {
            self.set_current_path.set(s.current_path);
            self.set_entries.set(s.entries);
            self.set_expanded_dirs.set(s.expanded_dirs);
            self.set_dir_children.set(s.dir_children);
            self.set_explorer_collapsed.set(s.explorer_collapsed);
            self.set_open_files.set(s.open_files);
            self.set_active_file.set(s.active_file);
            self.set_view_modes.set(s.view_modes);
            // Clear transient state
            self.set_inline_create.set(None);
            self.set_inline_confirm_delete.set(None);
            self.set_explorer_ctx_menu.set(None);
            self.set_save_status.set(None);
            self.set_hover_text.set(None);
            self.set_diagnostics.set(Vec::new());
            true
        } else {
            false
        }
    }

    /// Reset editor to initial empty state (for new project with no snapshot).
    pub fn reset_to_defaults(&self) {
        self.set_current_path.set(".".to_string());
        self.set_entries.set(Vec::new());
        self.set_expanded_dirs.set(HashSet::new());
        self.set_dir_children.set(HashMap::new());
        self.set_explorer_collapsed.set(false);
        self.set_open_files.set(Vec::new());
        self.set_active_file.set(None);
        self.set_view_modes.set(HashMap::new());
        self.set_inline_create.set(None);
        self.set_inline_confirm_delete.set(None);
        self.set_explorer_ctx_menu.set(None);
        self.set_save_status.set(None);
        self.set_hover_text.set(None);
        self.set_diagnostics.set(Vec::new());
        self.set_loading_dirs.set(HashSet::new());
    }
}
