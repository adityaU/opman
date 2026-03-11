use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::app::App;
use crate::theme::ThemeColors;

use super::FuzzyPickerState;

/// Render-only widget for the fuzzy picker overlay.
pub struct FuzzyPicker<'a> {
    app: &'a App,
}

impl<'a> FuzzyPicker<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn render_popup(self, area: Rect, buf: &mut Buffer) {
        let state = match &self.app.fuzzy_picker {
            Some(s) => s,
            None => return,
        };

        let theme = &self.app.theme;

        let popup_width = 80u16.min(area.width.saturating_sub(2));
        let max_list_h = area.height / 2;
        let popup_height = (max_list_h + 6).max(10).min(area.height.saturating_sub(2));
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height.min(area.height.saturating_sub(popup_y.saturating_sub(area.y))),
        };

        crate::ui::render_overlay_dim(area, buf);
        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(theme.background_panel));
        let panel_inner = block.inner(popup_area);
        block.render(popup_area, buf);

        let inner = Rect {
            x: panel_inner.x + 2,
            y: panel_inner.y + 1,
            width: panel_inner.width.saturating_sub(4),
            height: panel_inner.height.saturating_sub(1),
        };

        if inner.height < 5 {
            return;
        }

        let title_span = Span::styled(
            "Search",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        );
        let esc_span = Span::styled("esc", Style::default().fg(theme.text_muted));
        let title_line = Line::from(vec![
            title_span,
            Span::raw(" ".repeat((inner.width as usize).saturating_sub(6 + 3))),
            esc_span,
        ]);
        buf.set_line(inner.x, inner.y, &title_line, inner.width);

        let content_y = inner.y + 1;
        let content_height = inner.height.saturating_sub(1);

        if content_height < 3 {
            return;
        }

        let input_y = content_y + content_height.saturating_sub(1);
        let hint_y = content_y + content_height.saturating_sub(2);
        let separator_y = content_y + content_height.saturating_sub(3);
        let results_height = content_height.saturating_sub(3);

        render_input_line(buf, inner.x, input_y, inner.width, state, theme);
        render_hint_line(buf, inner.x, hint_y, inner.width, state, theme);
        render_separator(buf, inner.x, separator_y, inner.width, theme);

        if results_height > 0 {
            let results_area = Rect {
                x: inner.x,
                y: content_y,
                width: inner.width,
                height: results_height,
            };
            super::render_results::render_results(buf, results_area, state, theme);
        }
    }
}

fn render_input_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &FuzzyPickerState,
    theme: &ThemeColors,
) {
    // Prompt indicator
    let prompt = "> ";
    buf.set_string(
        x,
        y,
        prompt,
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD),
    );

    // Query text
    let query_x = x + prompt.len() as u16;
    buf.set_string(query_x, y, &state.query, Style::default().fg(theme.text));

    // Cursor
    let cursor_x = query_x + state.cursor_pos as u16;
    if cursor_x < x + width {
        let cursor_char = state.query[state.cursor_pos..]
            .chars()
            .next()
            .unwrap_or(' ');
        buf.set_string(
            cursor_x,
            y,
            cursor_char.to_string(),
            Style::default()
                .fg(theme.background)
                .bg(theme.text)
                .add_modifier(Modifier::BOLD),
        );
    }
}

fn render_hint_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &FuzzyPickerState,
    theme: &ThemeColors,
) {
    let (count_text, status_indicator) =
        if state.query.is_empty() && !state.existing_projects.is_empty() {
            let n = state.existing_projects.len();
            (
                format!("  {} project{}", n, if n == 1 { "" } else { "s" }),
                "",
            )
        } else {
            let matched = state.matched_count();
            let total = state.total_count();
            let indicator = if state.walk_complete() {
                ""
            } else {
                " (scanning...)"
            };
            (format!("  {}/{}", matched, total), indicator)
        };

    let spans = vec![
        Span::styled(
            format!("{}{}", count_text, status_indicator),
            Style::default().fg(theme.text_muted),
        ),
        Span::raw("  "),
        Span::styled("enter", Style::default().fg(theme.accent)),
        Span::styled(": select  ", Style::default().fg(theme.text_muted)),
        Span::styled("esc", Style::default().fg(theme.accent)),
        Span::styled(": cancel  ", Style::default().fg(theme.text_muted)),
        Span::styled("up/down", Style::default().fg(theme.accent)),
        Span::styled(": navigate", Style::default().fg(theme.text_muted)),
    ];

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    let hint_area = Rect {
        x,
        y,
        width,
        height: 1,
    };
    paragraph.render(hint_area, buf);
}

fn render_separator(buf: &mut Buffer, x: u16, y: u16, width: u16, theme: &ThemeColors) {
    let sep: String = "─".repeat(width as usize);
    buf.set_string(x, y, &sep, Style::default().fg(theme.border_subtle));
}
