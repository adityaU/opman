mod compute;
mod mouse;
mod navigation;
mod resize;
mod tree;
mod types;

use ratatui::layout::Rect;

use types::panel_index;
pub use types::{DragState, LayoutNode, PanelId, PanelRect, SeparatorRect, SplitDirection};

const MIN_PANEL_SIZE: u16 = 6;
const SEPARATOR_SIZE: u16 = 1;

pub struct LayoutManager {
    pub root: LayoutNode,
    pub focused: PanelId,
    pub panel_rects: Vec<PanelRect>,
    pub separator_rects: Vec<SeparatorRect>,
    pub drag_state: DragState,
    pub panel_visible: [bool; 5],
    pub last_area: Rect,
    /// Set to `true` whenever layout structure changes (visibility, resize, tree rebuild).
    /// The draw loop checks this + area change to decide whether to recompute rects.
    pub layout_dirty: bool,
}

impl LayoutManager {
    pub fn new() -> Self {
        Self {
            root: Self::default_layout(),
            focused: PanelId::TerminalPane,
            panel_rects: Vec::new(),
            separator_rects: Vec::new(),
            drag_state: DragState::None,
            panel_visible: [true, true, false, false, false],
            last_area: Rect::default(),
            layout_dirty: true,
        }
    }

    pub fn is_visible(&self, panel: PanelId) -> bool {
        self.panel_visible[panel_index(panel)]
    }

    pub fn set_visible(&mut self, panel: PanelId, visible: bool) {
        self.panel_visible[panel_index(panel)] = visible;
        self.rebuild_tree();
        if !visible && self.focused == panel {
            self.focused = PanelId::TerminalPane;
        }
    }

    pub fn toggle_visible(&mut self, panel: PanelId) {
        let idx = panel_index(panel);
        self.panel_visible[idx] = !self.panel_visible[idx];
        self.rebuild_tree();
        if !self.panel_visible[idx] && self.focused == panel {
            self.focused = PanelId::TerminalPane;
        }
        if self.panel_visible[idx] {
            self.focused = panel;
        }
    }
}
