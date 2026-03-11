use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::app::{App, SessionSelectorState};
use crate::theme::ThemeColors;

mod results;

pub(super) fn format_relative_time(updated: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let updated_secs = if updated > 10000000000 {
        updated / 1000
    } else {
        updated
    };

    let diff = now.saturating_sub(updated_secs);
    match diff {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{}m ago", diff / 60),
        3600..=86399 => format!("{}h ago", diff / 3600),
        _ => format!("{}d ago", diff / 86400),
    }
}

pub fn render_session_selector(app: &App, area: Rect, buf: &mut Buffer) {
    let state = match &app.session_selector {
        Some(s) => s,
        None => return,
    };

    let theme = &app.theme;

    let popup_width = 80u16.min(area.width.saturating_sub(2));
    let list_max = (area.height / 2).saturating_sub(6);
    let popup_height = (list_max + 6).max(8).min(area.height);
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    super::render_overlay_dim(area, buf);
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

    if inner.height < 4 {
        return;
    }

    let title_span = Span::styled(
        "Select Session",
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    );
    let esc_span = Span::styled("esc", Style::default().fg(theme.text_muted));
    let fill_len = (inner.width as usize).saturating_sub(14 + 3);
    let fill = " ".repeat(fill_len);
    let title_line = Line::from(vec![title_span, Span::raw(fill), esc_span]);
    buf.set_line(inner.x, inner.y, &title_line, inner.width);

    let input_y = inner.y + 1;
    let separator_y = input_y + 1;
    let hint_y = inner.y + inner.height.saturating_sub(1);
    let results_y = separator_y + 1;
    let results_height = hint_y.saturating_sub(results_y);

    // Input line
    render_input_line(buf, inner.x, input_y, inner.width, state, theme);

    // Separator
    let sep: String = "─".repeat(inner.width as usize);
    buf.set_string(
        inner.x,
        separator_y,
        &sep,
        Style::default().fg(theme.border_subtle),
    );

    // Results area
    if results_height > 0 {
        let results_area = Rect {
            x: inner.x,
            y: results_y,
            width: inner.width,
            height: results_height,
        };
        results::render_results(buf, results_area, state, theme);
    }

    // Hint line
    render_hint_line(buf, inner.x, hint_y, inner.width, theme);
}

fn render_input_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &SessionSelectorState,
    theme: &ThemeColors,
) {
    let prompt = "> ";
    buf.set_string(
        x,
        y,
        prompt,
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD),
    );

    let query_x = x + prompt.len() as u16;
    buf.set_string(query_x, y, &state.query, Style::default().fg(theme.text));

    // Block cursor
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

fn render_hint_line(buf: &mut Buffer, x: u16, y: u16, width: u16, theme: &ThemeColors) {
    let spans = vec![
        Span::styled("  ↑↓", Style::default().fg(theme.accent)),
        Span::styled(" navigate  ", Style::default().fg(theme.text_muted)),
        Span::styled("⏎", Style::default().fg(theme.accent)),
        Span::styled(" select  ", Style::default().fg(theme.text_muted)),
        Span::styled("esc", Style::default().fg(theme.accent)),
        Span::styled(" close", Style::default().fg(theme.text_muted)),
    ];

    let line = Line::from(spans);
    let hint_area = Rect {
        x,
        y,
        width,
        height: 1,
    };
    Paragraph::new(line).render(hint_area, buf);
}
