use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;

use crate::app::App;
use crate::theme::ThemeColors;

use super::layout_manager::{SeparatorRect, SplitDirection};

/// Must be called BEFORE rendering the popup panel on top.
pub fn render_overlay_dim(area: Rect, buf: &mut Buffer) {
    let dim_bg = Color::Rgb(10, 10, 10);
    let dim_fg = Color::Rgb(60, 60, 60);
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(dim_bg);
                cell.set_fg(dim_fg);
            }
        }
    }
}

/// Dim an unfocused panel by reducing the brightness of every cell.
/// Blends each color channel toward a dark base by the given factor
/// (0.0 = fully dark, 1.0 = no change).  Driven by the
/// `unfocused_dim_percent` setting (0-100, default 20).
pub(super) fn dim_panel(area: Rect, buf: &mut Buffer, factor: f32) {
    fn blend(c: Color, factor: f32) -> Color {
        match c {
            Color::Rgb(r, g, b) => Color::Rgb(
                (r as f32 * factor) as u8,
                (g as f32 * factor) as u8,
                (b as f32 * factor) as u8,
            ),
            Color::Reset => Color::Reset,
            other => other,
        }
    }
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_fg(blend(cell.fg, factor));
                cell.set_bg(blend(cell.bg, factor));
            }
        }
    }
}

#[allow(dead_code)]
pub fn render_pane_title_bar(
    buf: &mut Buffer,
    area: Rect,
    label: &str,
    is_focused: bool,
    theme: &ThemeColors,
) {
    if area.width == 0 {
        return;
    }
    let title_style = if is_focused {
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };
    let sep_style = Style::default().fg(theme.border_subtle);
    let title_text = format!(" {} ", label);
    let title_len = title_text.len();
    let remaining = (area.width as usize).saturating_sub(title_len + 1);
    let left = "─";
    let right = "─".repeat(remaining);

    let line = Line::from(vec![
        Span::styled(left, sep_style),
        Span::styled(title_text, title_style),
        Span::styled(right, sep_style),
    ]);
    Paragraph::new(line).render(
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
        buf,
    );
}

pub(super) fn render_separator(
    frame: &mut Frame,
    _app: &App,
    area: Rect,
    is_vertical: bool,
    color: Color,
) {
    let buf = frame.buffer_mut();
    let style = Style::default().fg(color);
    if is_vertical {
        for y in area.y..area.y + area.height {
            buf.set_string(area.x, y, "│", style);
        }
    } else {
        for x in area.x..area.x + area.width {
            buf.set_string(x, area.y, "─", style);
        }
    }
}

pub(super) fn render_separator_junctions(frame: &mut Frame, app: &App, seps: &[SeparatorRect]) {
    let buf = frame.buffer_mut();
    let style = Style::default().fg(app.theme.border_subtle);

    for a in seps {
        for b in seps {
            let (v, h) = if a.direction == SplitDirection::Horizontal
                && b.direction == SplitDirection::Vertical
            {
                (a, b)
            } else {
                continue;
            };

            let vx = v.rect.x;
            let vy_start = v.rect.y;
            let vy_end = v.rect.y + v.rect.height;
            let hy = h.rect.y;
            let hx_start = h.rect.x;
            let hx_end = h.rect.x + h.rect.width;

            if vx >= hx_start && vx < hx_end {
                if hy == vy_end {
                    buf.set_string(vx, hy, "┴", style);
                } else if hy + 1 == vy_start {
                    buf.set_string(vx, hy, "┬", style);
                } else if hy >= vy_start && hy < vy_end {
                    buf.set_string(vx, hy, "┼", style);
                }
            }
        }
    }
}
