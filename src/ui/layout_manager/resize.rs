use ratatui::layout::Rect;

use super::types::{DragState, LayoutNode, SplitDirection};
use super::LayoutManager;

impl LayoutManager {
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
    pub(crate) fn apply_resize_delta(
        &mut self,
        target_depth: usize,
        parent_idx: usize,
        ratio_delta: f64,
    ) {
        Self::apply_delta_recursive(&mut self.root, target_depth, 0, parent_idx, ratio_delta);
        self.layout_dirty = true;
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
}
