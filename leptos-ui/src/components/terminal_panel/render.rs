//! HTML rendering for the native terminal screen — cell styling, cursor, highlights.

/// Render the vt100 screen to a vector of HTML line fragments.
/// `highlights`: `(row, col_start, col_end, is_active)` for search matches.
pub fn render_screen_html(
    screen: &vt100::Screen,
    rows: usize,
    cols: usize,
    highlights: &[(usize, usize, usize, bool)],
) -> Vec<String> {
    let (cur_row, cur_col) = screen.cursor_position();

    let mut lines = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut html = String::with_capacity(cols * 20);
        let mut col = 0usize;
        while col < cols {
            let cell = screen.cell(row as u16, col as u16);
            let is_cursor = row == cur_row as usize && col == cur_col as usize;
            let ch = cell.map(|c| c.contents()).unwrap_or_default();
            let display = if ch.is_empty() { " " } else { &ch };

            let hl = highlights
                .iter()
                .find(|(r, cs, ce, _)| *r == row && col >= *cs && col < *ce);

            let mut style = String::new();
            if let Some(c) = cell {
                append_fg_style(&mut style, c.fgcolor());
                append_bg_style(&mut style, c.bgcolor());
                if c.bold() {
                    style.push_str("font-weight:bold;");
                }
                if c.italic() {
                    style.push_str("font-style:italic;");
                }
                if c.underline() {
                    style.push_str("text-decoration:underline;");
                }
            }
            if is_cursor {
                style.push_str(
                    "background:var(--color-text,#e0e0e0);\
                     color:var(--color-bg-panel,#1a1a1a);",
                );
            } else if let Some((_, _, _, is_active)) = hl {
                if *is_active {
                    style.push_str(
                        "background:var(--color-primary,#7c3aed);\
                         color:var(--color-text,#fff);",
                    );
                } else {
                    style.push_str(
                        "background:var(--color-warning,#e0af68);\
                         color:var(--color-bg-panel,#1a1a1a);",
                    );
                }
            }

            if style.is_empty() {
                html.push_str(&html_escape(display));
            } else {
                html.push_str("<span style=\"");
                html.push_str(&style);
                html.push_str("\">");
                html.push_str(&html_escape(display));
                html.push_str("</span>");
            }
            col += 1;
        }
        lines.push(html);
    }
    lines
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn append_fg_style(style: &mut String, color: vt100::Color) {
    match color {
        vt100::Color::Default => {}
        vt100::Color::Idx(i) => {
            if let Some(css) = ansi_idx_to_css(i) {
                style.push_str("color:");
                style.push_str(css);
                style.push(';');
            }
        }
        vt100::Color::Rgb(r, g, b) => {
            style.push_str(&format!("color:rgb({r},{g},{b});"));
        }
    }
}

fn append_bg_style(style: &mut String, color: vt100::Color) {
    match color {
        vt100::Color::Default => {}
        vt100::Color::Idx(i) => {
            if let Some(css) = ansi_idx_to_css(i) {
                style.push_str("background:");
                style.push_str(css);
                style.push(';');
            }
        }
        vt100::Color::Rgb(r, g, b) => {
            style.push_str(&format!("background:rgb({r},{g},{b});"));
        }
    }
}

/// Map standard 16 ANSI color indices to CSS custom properties
/// matching the terminal theme from the app's design system.
fn ansi_idx_to_css(idx: u8) -> Option<&'static str> {
    match idx {
        0 => Some("var(--color-bg-panel,#1a1a1a)"),
        1 => Some("var(--color-error,#f7768e)"),
        2 => Some("var(--color-success,#9ece6a)"),
        3 => Some("var(--color-warning,#e0af68)"),
        4 => Some("var(--color-secondary,#7aa2f7)"),
        5 => Some("var(--color-accent,#bb9af7)"),
        6 => Some("var(--color-info,#7dcfff)"),
        7 => Some("var(--color-text,#e0e0e0)"),
        8 => Some("var(--color-text-muted,#666)"),
        9 => Some("var(--color-error,#f7768e)"),
        10 => Some("var(--color-success,#9ece6a)"),
        11 => Some("var(--color-warning,#e0af68)"),
        12 => Some("var(--color-secondary,#7aa2f7)"),
        13 => Some("var(--color-accent,#bb9af7)"),
        14 => Some("var(--color-info,#7dcfff)"),
        15 => Some("var(--color-primary,#7c3aed)"),
        16..=255 => None,
        _ => None,
    }
}
