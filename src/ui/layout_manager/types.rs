use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelId {
    Sidebar,
    TerminalPane,
    NeovimPane,
    IntegratedTerminal,
    GitPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum LayoutNode {
    Leaf(PanelId),
    Split {
        direction: SplitDirection,
        children: Vec<(f64, LayoutNode)>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct PanelRect {
    pub panel: PanelId,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy)]
pub struct SeparatorRect {
    pub rect: Rect,
    pub direction: SplitDirection,
    pub parent_index: usize,
    pub depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragState {
    None,
    Dragging {
        separator_index: usize,
        last_pos: u16,
    },
}

pub(crate) struct LayoutRatios {
    pub sidebar: f64,
    pub terminal: f64,
    pub neovim: f64,
    pub top_vs_shell: f64,
    pub git_panel: f64,
}

pub(crate) fn panel_index(panel: PanelId) -> usize {
    match panel {
        PanelId::Sidebar => 0,
        PanelId::TerminalPane => 1,
        PanelId::NeovimPane => 2,
        PanelId::IntegratedTerminal => 3,
        PanelId::GitPanel => 4,
    }
}
