use ratatui::layout::Rect;

use super::types::{LayoutNode, PanelId, PanelRect, SeparatorRect, SplitDirection};
use super::{LayoutManager, MIN_PANEL_SIZE, SEPARATOR_SIZE};

impl LayoutManager {
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

    pub fn visible_panels(&self) -> Vec<PanelId> {
        self.panel_rects.iter().map(|p| p.panel).collect()
    }

    pub fn get_separator_rects(&self) -> &[SeparatorRect] {
        &self.separator_rects
    }
}
