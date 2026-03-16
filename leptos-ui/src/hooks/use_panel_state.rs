//! Panel state management.
//! Matches React `usePanelState.ts` — tracks sidebar, terminal, editor, git panels.
//! Includes per-project panel snapshot cache so switching projects restores panel state.

use leptos::prelude::*;
use std::collections::HashMap;

/// Which panel is focused.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FocusedPanel {
    Sidebar,
    Chat,
    Side,
}

/// Resizable panel dimensions (simplified for Phase 3).
#[derive(Clone, Copy)]
pub struct PanelSize {
    pub size: ReadSignal<f64>,
    pub set_size: WriteSignal<f64>,
}

impl PanelSize {
    pub fn new(initial: f64) -> Self {
        let (size, set_size) = signal(initial);
        Self { size, set_size }
    }
}

/// State for a toggleable panel (terminal, editor, git).
#[derive(Clone, Copy)]
pub struct TogglePanel {
    pub open: ReadSignal<bool>,
    pub set_open: WriteSignal<bool>,
    pub mounted: ReadSignal<bool>,
    set_mounted: WriteSignal<bool>,
}

impl TogglePanel {
    pub fn new(initial: bool) -> Self {
        let (open, set_open) = signal(initial);
        let (mounted, set_mounted) = signal(initial);

        // Once opened, stay mounted (guard prevents re-notification when already true)
        Effect::new(move |_| {
            if open.get() && !mounted.get_untracked() {
                set_mounted.set(true);
            }
        });

        Self {
            open,
            set_open,
            mounted,
            set_mounted,
        }
    }

    pub fn toggle(&self) {
        self.set_open.update(|v| *v = !*v);
    }

    pub fn close(&self) {
        self.set_open.set(false);
    }
}

/// Initial panel configuration.
pub struct PanelConfig {
    pub sidebar: bool,
    pub terminal: bool,
    pub editor: bool,
    pub git: bool,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            sidebar: true,
            terminal: false,
            editor: false,
            git: false,
        }
    }
}

/// Snapshot of panel open/close state + sizes for per-project caching.
#[derive(Clone, Debug)]
struct PanelSnapshot {
    sidebar_open: bool,
    terminal_open: bool,
    editor_open: bool,
    git_open: bool,
    focused: FocusedPanel,
    sidebar_width: f64,
    terminal_height: f64,
    side_panel_width: f64,
}

/// Full panel state — sidebar + terminal + editor + git + focus + sizes.
#[derive(Clone, Copy)]
pub struct PanelState {
    // Sidebar
    pub sidebar_open: ReadSignal<bool>,
    pub set_sidebar_open: WriteSignal<bool>,
    pub sidebar_size: PanelSize,

    // Terminal
    pub terminal: TogglePanel,
    pub terminal_size: PanelSize,

    // Editor
    pub editor: TogglePanel,

    // Git
    pub git: TogglePanel,

    // Side panel (editor/git combined)
    pub side_panel_size: PanelSize,

    // Focus
    pub focused: ReadSignal<FocusedPanel>,
    pub set_focused: WriteSignal<FocusedPanel>,

    // Per-project snapshot cache
    project_snapshots: StoredValue<HashMap<usize, PanelSnapshot>>,
}

impl PanelState {
    pub fn toggle_sidebar(&self) {
        self.set_sidebar_open.update(|v| *v = !*v);
    }

    pub fn has_side_panel(&self) -> bool {
        self.editor.open.get_untracked() || self.git.open.get_untracked()
    }

    pub fn focus_sidebar(&self) {
        self.set_focused.set(FocusedPanel::Sidebar);
    }

    pub fn focus_chat(&self) {
        self.set_focused.set(FocusedPanel::Chat);
    }

    pub fn focus_side(&self) {
        self.set_focused.set(FocusedPanel::Side);
    }

    /// Save current panel open/focus state + sizes for the given project index.
    pub fn save_for_project(&self, project_idx: usize) {
        let snap = PanelSnapshot {
            sidebar_open: self.sidebar_open.get_untracked(),
            terminal_open: self.terminal.open.get_untracked(),
            editor_open: self.editor.open.get_untracked(),
            git_open: self.git.open.get_untracked(),
            focused: self.focused.get_untracked(),
            sidebar_width: self.sidebar_size.size.get_untracked(),
            terminal_height: self.terminal_size.size.get_untracked(),
            side_panel_width: self.side_panel_size.size.get_untracked(),
        };
        self.project_snapshots.update_value(|map| {
            map.insert(project_idx, snap);
        });
    }

    /// Restore panel open/focus state + sizes for the given project index.
    /// If no snapshot exists, panels are left as-is (defaults).
    pub fn restore_for_project(&self, project_idx: usize) {
        let snap = self
            .project_snapshots
            .with_value(|map| map.get(&project_idx).cloned());
        if let Some(s) = snap {
            self.set_sidebar_open.set(s.sidebar_open);
            self.terminal.set_open.set(s.terminal_open);
            self.editor.set_open.set(s.editor_open);
            self.git.set_open.set(s.git_open);
            self.set_focused.set(s.focused);
            self.sidebar_size.set_size.set(s.sidebar_width);
            self.terminal_size.set_size.set(s.terminal_height);
            self.side_panel_size.set_size.set(s.side_panel_width);
        }
    }
}

/// Create the full panel state. Call once at the layout level.
pub fn use_panel_state(config: PanelConfig) -> PanelState {
    let (sidebar_open, set_sidebar_open) = signal(config.sidebar);
    let sidebar_size = PanelSize::new(280.0);

    let terminal = TogglePanel::new(config.terminal);
    let terminal_size = PanelSize::new(250.0);

    let editor = TogglePanel::new(config.editor);
    let git = TogglePanel::new(config.git);
    let side_panel_size = PanelSize::new(500.0);

    let (focused, set_focused) = signal(FocusedPanel::Chat);

    let project_snapshots = StoredValue::new(HashMap::<usize, PanelSnapshot>::new());

    PanelState {
        sidebar_open,
        set_sidebar_open,
        sidebar_size,
        terminal,
        terminal_size,
        editor,
        git,
        side_panel_size,
        focused,
        set_focused,
        project_snapshots,
    }
}
