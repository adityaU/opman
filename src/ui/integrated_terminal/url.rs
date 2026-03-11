use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

pub(super) fn render_url_underlines(buf: &mut Buffer, area: Rect, screen: &vt100::Screen) {
    let rows = area.height as usize;
    let cols = area.width as usize;
    for row in 0..rows {
        let line = screen.contents_between(row as u16, 0, row as u16, cols as u16);
        let mut search_from = 0;
        while search_from < line.len() {
            let remaining = &line[search_from..];
            let url_start = if let Some(pos) = remaining.find("https://") {
                Some(pos)
            } else if let Some(pos) = remaining.find("http://") {
                Some(pos)
            } else if let Some(pos) = remaining.find("ftp://") {
                Some(pos)
            } else {
                None
            };
            if let Some(start) = url_start {
                let abs_start = search_from + start;
                let url_bytes = remaining[start..].as_bytes();
                let mut end = 0;
                for &b in url_bytes {
                    if b == b' '
                        || b == b'\t'
                        || b == b'"'
                        || b == b'\''
                        || b == b'>'
                        || b == b'<'
                        || b == b'|'
                        || b == b'{'
                        || b == b'}'
                        || b == b'`'
                    {
                        break;
                    }
                    end += 1;
                }
                if end > 8 {
                    let url_style = Style::default()
                        .fg(Color::Rgb(100, 149, 237))
                        .add_modifier(Modifier::UNDERLINED);
                    for col in abs_start..abs_start + end {
                        if col < cols {
                            let x = area.x + col as u16;
                            let y = area.y + row as u16;
                            if x < area.x + area.width && y < area.y + area.height {
                                let cell = buf.cell_mut(ratatui::layout::Position::new(x, y));
                                if let Some(cell) = cell {
                                    cell.set_style(url_style);
                                }
                            }
                        }
                    }
                }
                search_from = abs_start + end;
            } else {
                break;
            }
        }
    }
}
