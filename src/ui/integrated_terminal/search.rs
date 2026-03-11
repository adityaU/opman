use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::app::App;

pub(super) fn render_search_highlights(app: &App, content_area: Rect, buf: &mut Buffer) {
    if let Some(ref search) = app.terminal_search {
        if search.query.is_empty() {
            return;
        }

        let match_bg = Color::Rgb(100, 100, 40);
        let current_match_bg = Color::Rgb(200, 150, 0);
        let match_fg = Color::Rgb(255, 255, 255);

        for (i, &(row, col, len)) in search.matches.iter().enumerate() {
            let is_current = i == search.current_match;
            let bg = if is_current {
                current_match_bg
            } else {
                match_bg
            };

            for offset in 0..len {
                let x = content_area.x + (col + offset) as u16;
                let y = content_area.y + row as u16;
                if x < content_area.right() && y < content_area.bottom() {
                    let cell = buf.cell_mut((x, y)).expect("cell in bounds");
                    cell.set_bg(bg);
                    cell.set_fg(match_fg);
                    if is_current {
                        cell.set_style(
                            Style::default()
                                .fg(match_fg)
                                .bg(bg)
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                }
            }
        }
    }
}

pub(super) fn render_search_bar(app: &App, content_area: Rect, buf: &mut Buffer) {
    if let Some(ref search) = app.terminal_search {
        let bar_y = content_area.y;
        let bar_width = content_area.width.min(60);
        let bar_x = content_area.right().saturating_sub(bar_width);

        let bar_bg = Color::Rgb(50, 50, 50);
        let bar_fg = Color::Rgb(220, 220, 220);
        let input_fg = Color::Rgb(255, 255, 255);

        for x in bar_x..content_area.right() {
            if bar_y < content_area.bottom() {
                let cell = buf.cell_mut((x, bar_y)).expect("cell in bounds");
                cell.set_bg(bar_bg);
                cell.set_fg(bar_fg);
                cell.set_symbol(" ");
            }
        }

        let label = " Find: ";
        buf.set_string(bar_x, bar_y, label, Style::default().fg(bar_fg).bg(bar_bg));

        let query_x = bar_x + label.len() as u16;
        let max_query_len = (content_area
            .right()
            .saturating_sub(query_x)
            .saturating_sub(15)) as usize;
        let display_query = if search.query.len() > max_query_len {
            &search.query[search.query.len() - max_query_len..]
        } else {
            &search.query
        };
        buf.set_string(
            query_x,
            bar_y,
            display_query,
            Style::default()
                .fg(input_fg)
                .bg(bar_bg)
                .add_modifier(Modifier::BOLD),
        );

        let cursor_x = query_x + display_query.len() as u16;
        if cursor_x < content_area.right().saturating_sub(15) {
            buf.set_string(
                cursor_x,
                bar_y,
                "▏",
                Style::default().fg(Color::Rgb(255, 255, 255)).bg(bar_bg),
            );
        }

        let match_info = if search.matches.is_empty() {
            if search.query.is_empty() {
                String::new()
            } else {
                " No matches ".to_string()
            }
        } else {
            format!(" {}/{} ", search.current_match + 1, search.matches.len())
        };

        if !match_info.is_empty() {
            let info_x = content_area.right().saturating_sub(match_info.len() as u16);
            buf.set_string(
                info_x,
                bar_y,
                &match_info,
                Style::default().fg(bar_fg).bg(bar_bg),
            );
        }
    }
}
