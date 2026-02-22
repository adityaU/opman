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

pub struct LayoutManager {
    pub root: LayoutNode,
    pub focused: PanelId,
    pub panel_rects: Vec<PanelRect>,
    pub separator_rects: Vec<SeparatorRect>,
    pub drag_state: DragState,
    pub panel_visible: [bool; 5],
    pub last_area: Rect,
}

const MIN_PANEL_SIZE: u16 = 6;
const SEPARATOR_SIZE: u16 = 1;

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
        }
    }

    fn default_layout() -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            children: vec![
                (0.22, LayoutNode::Leaf(PanelId::Sidebar)),
                (0.78, LayoutNode::Leaf(PanelId::TerminalPane)),
            ],
        }
    }

    #[allow(dead_code)]
    fn default_layout_with_shell() -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Vertical,
            children: vec![
                (
                    0.67,
                    LayoutNode::Split {
                        direction: SplitDirection::Horizontal,
                        children: vec![
                            (0.22, LayoutNode::Leaf(PanelId::Sidebar)),
                            (0.78, LayoutNode::Leaf(PanelId::TerminalPane)),
                        ],
                    },
                ),
                (0.33, LayoutNode::Leaf(PanelId::IntegratedTerminal)),
            ],
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

    fn rebuild_tree(&mut self) {
        let sidebar = self.panel_visible[0];
        let terminal = self.panel_visible[1];
        let neovim = self.panel_visible[2];
        let shell = self.panel_visible[3];
        let git_panel = self.panel_visible[4];

        let ratios = self.extract_ratios();

        let top_children: Vec<(f64, LayoutNode)> = {
            let mut v = Vec::new();
            if sidebar {
                v.push((ratios.sidebar, LayoutNode::Leaf(PanelId::Sidebar)));
            }
            if terminal {
                v.push((ratios.terminal, LayoutNode::Leaf(PanelId::TerminalPane)));
            }
            if neovim {
                v.push((ratios.neovim, LayoutNode::Leaf(PanelId::NeovimPane)));
            }
            if git_panel {
                v.push((ratios.git_panel, LayoutNode::Leaf(PanelId::GitPanel)));
            }
            v
        };

        let top = if top_children.len() == 1 {
            top_children.into_iter().next().unwrap().1
        } else if top_children.is_empty() {
            if shell {
                self.root = LayoutNode::Leaf(PanelId::IntegratedTerminal);
            } else {
                self.root = LayoutNode::Leaf(PanelId::TerminalPane);
            }
            return;
        } else {
            LayoutNode::Split {
                direction: SplitDirection::Horizontal,
                children: top_children,
            }
        };

        if shell {
            self.root = LayoutNode::Split {
                direction: SplitDirection::Vertical,
                children: vec![
                    (ratios.top_vs_shell, top),
                    (
                        1.0 - ratios.top_vs_shell,
                        LayoutNode::Leaf(PanelId::IntegratedTerminal),
                    ),
                ],
            };
        } else {
            self.root = top;
        }
    }

    fn extract_ratios(&self) -> LayoutRatios {
        let mut ratios = LayoutRatios {
            sidebar: 0.22,
            terminal: 0.78,
            neovim: 0.39,
            top_vs_shell: 0.67,
            git_panel: 0.39,
        };

        match &self.root {
            LayoutNode::Split {
                direction: SplitDirection::Vertical,
                children,
            } if children.len() == 2 => {
                ratios.top_vs_shell = children[0].0;
                if let LayoutNode::Split {
                    direction: SplitDirection::Horizontal,
                    children: inner,
                } = &children[0].1
                {
                    Self::extract_h_ratios(inner, &mut ratios);
                }
            }
            LayoutNode::Split {
                direction: SplitDirection::Horizontal,
                children,
            } => {
                Self::extract_h_ratios(children, &mut ratios);
            }
            _ => {}
        }

        ratios
    }

    fn extract_h_ratios(children: &[(f64, LayoutNode)], ratios: &mut LayoutRatios) {
        for (r, node) in children {
            match node {
                LayoutNode::Leaf(PanelId::Sidebar) => ratios.sidebar = *r,
                LayoutNode::Leaf(PanelId::TerminalPane) => ratios.terminal = *r,
                LayoutNode::Leaf(PanelId::NeovimPane) => ratios.neovim = *r,
                LayoutNode::Leaf(PanelId::GitPanel) => ratios.git_panel = *r,
                _ => {}
            }
        }
    }

    pub fn compute_rects(&mut self, area: Rect) {
        self.last_area = area;
        self.panel_rects.clear();
        self.separator_rects.clear();
        self.compute_node_rects(&self.root.clone(), area, 0);
    }

    fn compute_node_rects(&mut self, node: &LayoutNode, area: Rect, depth: usize) {
        match node {
            LayoutNode::Leaf(panel) => {
                self.panel_rects.push(PanelRect {
                    panel: *panel,
                    rect: area,
                });
            }
            LayoutNode::Split {
                direction,
                children,
            } => {
                let total_seps = (children.len().saturating_sub(1)) as u16 * SEPARATOR_SIZE;
                let available = match direction {
                    SplitDirection::Horizontal => area.width.saturating_sub(total_seps),
                    SplitDirection::Vertical => area.height.saturating_sub(total_seps),
                };

                let total_ratio: f64 = children.iter().map(|(r, _)| r).sum();
                let mut pos = match direction {
                    SplitDirection::Horizontal => area.x,
                    SplitDirection::Vertical => area.y,
                };

                for (i, (ratio, child)) in children.iter().enumerate() {
                    let normalized = ratio / total_ratio;
                    let size = if i == children.len() - 1 {
                        match direction {
                            SplitDirection::Horizontal => (area.x + area.width).saturating_sub(pos),
                            SplitDirection::Vertical => (area.y + area.height).saturating_sub(pos),
                        }
                    } else {
                        (available as f64 * normalized)
                            .round()
                            .max(MIN_PANEL_SIZE as f64) as u16
                    };

                    let child_rect = match direction {
                        SplitDirection::Horizontal => Rect::new(pos, area.y, size, area.height),
                        SplitDirection::Vertical => Rect::new(area.x, pos, area.width, size),
                    };

                    self.compute_node_rects(child, child_rect, depth + 1);
                    pos += size;

                    if i < children.len() - 1 {
                        let sep_rect = match direction {
                            SplitDirection::Horizontal => {
                                Rect::new(pos, area.y, SEPARATOR_SIZE, area.height)
                            }
                            SplitDirection::Vertical => {
                                Rect::new(area.x, pos, area.width, SEPARATOR_SIZE)
                            }
                        };
                        self.separator_rects.push(SeparatorRect {
                            rect: sep_rect,
                            direction: *direction,
                            parent_index: i,
                            depth,
                        });
                        pos += SEPARATOR_SIZE;
                    }
                }
            }
        }
    }

    pub fn panel_rect(&self, panel: PanelId) -> Option<Rect> {
        self.panel_rects
            .iter()
            .find(|p| p.panel == panel)
            .map(|p| p.rect)
    }

    pub fn panel_at(&self, x: u16, y: u16) -> Option<PanelId> {
        self.panel_rects
            .iter()
            .find(|p| {
                x >= p.rect.x
                    && x < p.rect.x + p.rect.width
                    && y >= p.rect.y
                    && y < p.rect.y + p.rect.height
            })
            .map(|p| p.panel)
    }

    pub fn separator_at(&self, x: u16, y: u16) -> Option<usize> {
        for (i, sep) in self.separator_rects.iter().enumerate() {
            let r = sep.rect;
            let hit = match sep.direction {
                SplitDirection::Horizontal => {
                    x >= r.x.saturating_sub(1)
                        && x <= r.x + r.width
                        && y >= r.y
                        && y < r.y + r.height
                }
                SplitDirection::Vertical => {
                    y >= r.y.saturating_sub(1)
                        && y <= r.y + r.height
                        && x >= r.x
                        && x < r.x + r.width
                }
            };
            if hit {
                return Some(i);
            }
        }
        None
    }

    pub fn start_drag(&mut self, sep_index: usize, pos: u16) {
        self.drag_state = DragState::Dragging {
            separator_index: sep_index,
            last_pos: pos,
        };
    }

    pub fn update_drag(&mut self, current_pos: u16, area: Rect) {
        if let DragState::Dragging {
            separator_index,
            last_pos,
        } = self.drag_state
        {
            let delta = current_pos as i32 - last_pos as i32;
            if delta == 0 {
                return;
            }

            if let Some(sep) = self.separator_rects.get(separator_index) {
                let depth = sep.depth;
                let parent_idx = sep.parent_index;
                let direction = sep.direction;

                let available = match direction {
                    SplitDirection::Horizontal => area.width,
                    SplitDirection::Vertical => area.height,
                };

                if available > 0 {
                    let ratio_delta = delta as f64 / available as f64;
                    self.apply_resize_delta(depth, parent_idx, ratio_delta);
                }
            }

            self.drag_state = DragState::Dragging {
                separator_index,
                last_pos: current_pos,
            };
        }
    }

    /// Delta-based resize: adjusts ratios by delta rather than absolute position,
    /// preventing jerk when the click doesn't exactly land on the separator center.
    fn apply_resize_delta(&mut self, target_depth: usize, parent_idx: usize, ratio_delta: f64) {
        Self::apply_delta_recursive(&mut self.root, target_depth, 0, parent_idx, ratio_delta);
    }

    fn apply_delta_recursive(
        node: &mut LayoutNode,
        target_depth: usize,
        current_depth: usize,
        parent_idx: usize,
        ratio_delta: f64,
    ) {
        if let LayoutNode::Split { children, .. } = node {
            if current_depth == target_depth {
                if parent_idx < children.len() - 1 {
                    let total_ratio: f64 = children.iter().map(|(r, _)| r).sum();
                    let min_ratio = 0.05;

                    let new_left = children[parent_idx].0 + ratio_delta;
                    let new_right = children[parent_idx + 1].0 - ratio_delta;

                    if new_left >= min_ratio * total_ratio && new_right >= min_ratio * total_ratio {
                        children[parent_idx].0 = new_left;
                        children[parent_idx + 1].0 = new_right;
                    }
                }
                return;
            }

            for (_, child) in children.iter_mut() {
                Self::apply_delta_recursive(
                    child,
                    target_depth,
                    current_depth + 1,
                    parent_idx,
                    ratio_delta,
                );
            }
        }
    }

    pub fn end_drag(&mut self) {
        self.drag_state = DragState::None;
    }

    pub fn focus_direction(&self, dx: i32, dy: i32) -> Option<PanelId> {
        let cur = self.panel_rect(self.focused)?;

        let mut best: Option<(PanelId, i32, i32)> = None;

        for pr in &self.panel_rects {
            if pr.panel == self.focused || !self.is_visible(pr.panel) {
                continue;
            }
            let r = pr.rect;

            if dy != 0 {
                let in_dir = if dy < 0 {
                    (r.y as i32) < cur.y as i32
                } else {
                    (r.y as i32 + r.height as i32) > (cur.y as i32 + cur.height as i32)
                };
                if !in_dir {
                    continue;
                }
                let h_overlap = (r.x as i32) < (cur.x as i32 + cur.width as i32)
                    && (r.x as i32 + r.width as i32) > cur.x as i32;
                if !h_overlap {
                    continue;
                }
                let vdist = if dy < 0 {
                    cur.y as i32 - (r.y as i32 + r.height as i32)
                } else {
                    r.y as i32 - (cur.y as i32 + cur.height as i32)
                };
                let secondary = r.x as i32;
                if let Some((_, bd, bs)) = best {
                    if vdist < bd || (vdist == bd && secondary < bs) {
                        best = Some((pr.panel, vdist, secondary));
                    }
                } else {
                    best = Some((pr.panel, vdist, secondary));
                }
            } else if dx != 0 {
                let in_dir = if dx < 0 {
                    (r.x as i32) < cur.x as i32
                } else {
                    (r.x as i32 + r.width as i32) > (cur.x as i32 + cur.width as i32)
                };
                if !in_dir {
                    continue;
                }
                let v_overlap = (r.y as i32) < (cur.y as i32 + cur.height as i32)
                    && (r.y as i32 + r.height as i32) > cur.y as i32;
                if !v_overlap {
                    continue;
                }
                let hdist = if dx < 0 {
                    cur.x as i32 - (r.x as i32 + r.width as i32)
                } else {
                    r.x as i32 - (cur.x as i32 + cur.width as i32)
                };
                let secondary = r.y as i32;
                if let Some((_, bd, bs)) = best {
                    if hdist < bd || (hdist == bd && secondary < bs) {
                        best = Some((pr.panel, hdist, secondary));
                    }
                } else {
                    best = Some((pr.panel, hdist, secondary));
                }
            }
        }

        best.map(|(p, _, _)| p)
    }

    pub fn swap_focused_with(&mut self, target: PanelId) {
        if self.focused == target {
            return;
        }
        Self::swap_panels_in_node(&mut self.root, self.focused, target);
    }

    fn swap_panels_in_node(node: &mut LayoutNode, a: PanelId, b: PanelId) {
        match node {
            LayoutNode::Leaf(panel) => {
                if *panel == a {
                    *panel = b;
                } else if *panel == b {
                    *panel = a;
                }
            }
            LayoutNode::Split { children, .. } => {
                for (_, child) in children {
                    Self::swap_panels_in_node(child, a, b);
                }
            }
        }
    }

    pub fn visible_panels(&self) -> Vec<PanelId> {
        self.panel_rects.iter().map(|p| p.panel).collect()
    }

    pub fn navigate_left(&mut self) {
        if let Some(target) = self.focus_direction(-1, 0) {
            self.focused = target;
        }
    }

    pub fn navigate_right(&mut self) {
        if let Some(target) = self.focus_direction(1, 0) {
            self.focused = target;
        }
    }

    pub fn navigate_up(&mut self) {
        if let Some(target) = self.focus_direction(0, -1) {
            self.focused = target;
        }
    }

    pub fn navigate_down(&mut self) {
        if let Some(target) = self.focus_direction(0, 1) {
            self.focused = target;
        }
    }

    pub fn swap_focused_with_next(&mut self) {
        let panels = self.visible_panels();
        if panels.len() < 2 {
            return;
        }
        if let Some(pos) = panels.iter().position(|p| *p == self.focused) {
            let next = panels[(pos + 1) % panels.len()];
            self.swap_focused_with(next);
        }
    }

    pub fn get_separator_rects(&self) -> &[SeparatorRect] {
        &self.separator_rects
    }

    pub fn resize_focused(&mut self, dx: i16, dy: i16) {
        let step: f64 = 0.02;
        let focused_rect = match self.panel_rect(self.focused) {
            Some(r) => r,
            None => return,
        };

        for sep in &self.separator_rects.clone() {
            match sep.direction {
                SplitDirection::Horizontal if dx != 0 => {
                    let sep_x = sep.rect.x;
                    let y_overlap = sep.rect.y < focused_rect.y + focused_rect.height
                        && sep.rect.y + sep.rect.height > focused_rect.y;
                    if !y_overlap {
                        continue;
                    }

                    if dx > 0 && sep_x == focused_rect.x + focused_rect.width {
                        self.apply_resize_delta(sep.depth, sep.parent_index, step);
                        return;
                    }
                    if dx < 0 && sep_x + sep.rect.width == focused_rect.x {
                        self.apply_resize_delta(sep.depth, sep.parent_index, -step);
                        return;
                    }
                    if dx > 0 && sep_x + sep.rect.width == focused_rect.x {
                        self.apply_resize_delta(sep.depth, sep.parent_index, step);
                        return;
                    }
                    if dx < 0 && sep_x == focused_rect.x + focused_rect.width {
                        self.apply_resize_delta(sep.depth, sep.parent_index, -step);
                        return;
                    }
                }
                SplitDirection::Vertical if dy != 0 => {
                    let sep_y = sep.rect.y;
                    let x_overlap = sep.rect.x < focused_rect.x + focused_rect.width
                        && sep.rect.x + sep.rect.width > focused_rect.x;
                    if !x_overlap {
                        continue;
                    }

                    if dy > 0 && sep_y == focused_rect.y + focused_rect.height {
                        self.apply_resize_delta(sep.depth, sep.parent_index, step);
                        return;
                    }
                    if dy < 0 && sep_y + sep.rect.height == focused_rect.y {
                        self.apply_resize_delta(sep.depth, sep.parent_index, -step);
                        return;
                    }
                    if dy > 0 && sep_y + sep.rect.height == focused_rect.y {
                        self.apply_resize_delta(sep.depth, sep.parent_index, step);
                        return;
                    }
                    if dy < 0 && sep_y == focused_rect.y + focused_rect.height {
                        self.apply_resize_delta(sep.depth, sep.parent_index, -step);
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn handle_mouse(
        &mut self,
        event: crossterm::event::MouseEvent,
        area: Rect,
    ) -> Option<PanelId> {
        use crossterm::event::{MouseButton, MouseEventKind};
        let x = event.column;
        let y = event.row;

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(sep_idx) = self.separator_at(x, y) {
                    let sep = &self.separator_rects[sep_idx];
                    let pos = match sep.direction {
                        SplitDirection::Vertical => y,
                        SplitDirection::Horizontal => x,
                    };
                    self.start_drag(sep_idx, pos);
                    None
                } else if let Some(panel) = self.panel_at(x, y) {
                    self.focused = panel;
                    None
                } else {
                    None
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let DragState::Dragging {
                    separator_index, ..
                } = self.drag_state
                {
                    if let Some(sep) = self.separator_rects.get(separator_index) {
                        let pos = match sep.direction {
                            SplitDirection::Vertical => y,
                            SplitDirection::Horizontal => x,
                        };
                        self.update_drag(pos, area);
                    }
                }
                None
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.end_drag();
                None
            }
            MouseEventKind::ScrollDown
            | MouseEventKind::ScrollUp
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight => {
                if let Some(panel) = self.panel_at(x, y) {
                    self.focused = panel;
                }
                None
            }
            _ => None,
        }
    }
}

struct LayoutRatios {
    sidebar: f64,
    terminal: f64,
    neovim: f64,
    top_vs_shell: f64,
    git_panel: f64,
}

fn panel_index(panel: PanelId) -> usize {
    match panel {
        PanelId::Sidebar => 0,
        PanelId::TerminalPane => 1,
        PanelId::NeovimPane => 2,
        PanelId::IntegratedTerminal => 3,
        PanelId::GitPanel => 4,
    }
}
