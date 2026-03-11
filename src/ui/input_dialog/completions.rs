use std::cmp::min;
use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::InputDialog;

impl<'a> InputDialog<'a> {
    pub(super) fn render_completions(&self, area: Rect, buf: &mut Buffer) {
        let total = self.app.completions.len();
        let visible = area.height as usize;
        let selected = self.app.completion_selected;

        let scroll_offset = if selected >= visible {
            selected - visible + 1
        } else {
            0
        };

        let end = min(scroll_offset + visible, total);

        for (i, idx) in (scroll_offset..end).enumerate() {
            let row_area = Rect::new(area.x, area.y + i as u16, area.width, 1);
            let path_str = &self.app.completions[idx];

            let basename = Path::new(path_str)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path_str.clone());

            let parent = Path::new(path_str)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let available_width = area.width as usize;
            let parent_display = if parent.len() + basename.len() + 3 <= available_width {
                format!("  {}", parent)
            } else {
                String::new()
            };

            let is_selected = idx == selected;

            let (name_style, parent_style) = if is_selected {
                (
                    Style::default()
                        .bg(self.app.theme.primary)
                        .fg(self.app.theme.background)
                        .add_modifier(Modifier::BOLD),
                    Style::default()
                        .bg(self.app.theme.primary)
                        .fg(self.app.theme.text_muted),
                )
            } else {
                (
                    Style::default().fg(self.app.theme.text),
                    Style::default().fg(self.app.theme.text_muted),
                )
            };

            let mut spans = vec![Span::styled(format!(" {}", basename), name_style)];
            if !parent_display.is_empty() {
                spans.push(Span::styled(parent_display, parent_style));
            }

            if is_selected {
                let used: usize = spans.iter().map(|s| s.content.len()).sum();
                if used < available_width {
                    spans.push(Span::styled(
                        " ".repeat(available_width - used),
                        Style::default().bg(self.app.theme.primary),
                    ));
                }
            }

            let line = Line::from(spans);
            Paragraph::new(line).render(row_area, buf);
        }

        if total > visible {
            let indicator = format!(" [{}/{}]", selected + 1, total);
            let indicator_area = Rect::new(
                area.x + area.width.saturating_sub(indicator.len() as u16),
                area.y + area.height.saturating_sub(1),
                indicator.len() as u16,
                1,
            );
            let span = Span::styled(indicator, Style::default().fg(self.app.theme.text_muted));
            Paragraph::new(Line::from(span)).render(indicator_area, buf);
        }
    }
}
