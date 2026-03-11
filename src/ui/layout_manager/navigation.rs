use super::types::{LayoutNode, PanelId};
use super::LayoutManager;

impl LayoutManager {
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

    #[allow(dead_code)]
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
}
