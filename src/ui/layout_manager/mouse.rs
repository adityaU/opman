use ratatui::layout::Rect;

use super::types::{DragState, PanelId, SplitDirection};
use super::LayoutManager;

impl LayoutManager {
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
