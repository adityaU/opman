mod build;
mod search_panel;

pub use search_panel::SessionSearchPanel;

use std::cmp::min;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Widget};

use ratatui::style::Color;

use crate::app::App;

pub(crate) fn lerp_color(from: Color, to: Color, t: f64) -> Color {
    let (r1, g1, b1) = match from {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    };
    let (r2, g2, b2) = match to {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (255, 255, 255),
    };
    Color::Rgb(
        (r1 as f64 + (r2 as f64 - r1 as f64) * t) as u8,
        (g1 as f64 + (g2 as f64 - g1 as f64) * t) as u8,
        (b1 as f64 + (b2 as f64 - b1 as f64) * t) as u8,
    )
}

/// Sidebar widget that displays the project list with sessions,
/// and an optional keyboard shortcuts help panel at the bottom.
pub struct Sidebar<'a> {
    app: &'a App,
}

impl<'a> Sidebar<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    /// Apply cursor (j/k highlight) background to an existing style.
    pub(super) fn with_cursor_bg(&self, style: Style) -> Style {
        style.bg(self.app.theme.background_element)
    }
}

impl<'a> Widget for Sidebar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut items = self.build_items();

        if let Some(idx) = self.app.confirm_delete {
            let name = self
                .app
                .projects
                .get(idx)
                .map(|p| p.name.as_str())
                .unwrap_or("?");
            items.push(ListItem::new(Line::from("")));
            items.push(ListItem::new(Line::from(Span::styled(
                format!("  Delete \"{}\"? (y/N)", name),
                Style::default().fg(self.app.theme.warning),
            ))));
        } else {
            items.push(ListItem::new(Line::from("")));
            items.push(ListItem::new(Line::from(Span::styled(
                "  ? for shortcuts",
                Style::default().fg(self.app.theme.text_muted),
            ))));
        }

        // Scrolling follows the cursor (j/k position)
        let max_visible = area.height.saturating_sub(1) as usize;
        let total_items = items.len();
        let sidebar_cursor = self.app.sidebar_cursor;

        let scroll_offset = if sidebar_cursor >= max_visible {
            sidebar_cursor - max_visible + 1
        } else {
            0
        };

        let end = min(scroll_offset + max_visible, total_items);
        let visible_items: Vec<ListItem> = items
            .into_iter()
            .skip(scroll_offset)
            .take(end - scroll_offset)
            .collect();

        let list = List::new(visible_items);
        Widget::render(list, area, buf);
    }
}
