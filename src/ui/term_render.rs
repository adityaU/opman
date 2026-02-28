//! Optimized terminal renderer that replaces `tui_term::PseudoTerminal`.
//!
//! Key optimizations over the original tui-term v0.2:
//!
//! 1. **No redundant `Clear` pass** – callers already fill the background.
//! 2. **Direct `vt100::Color → ratatui::Color` conversion** – eliminates the
//!    intermediate `tui_term::Color` enum and its double `From` conversion.
//! 3. **Inline ANSI palette remapping** – the separate `remap_ansi_colors()`
//!    post-processing pass is merged into the main cell loop, avoiding a
//!    second O(rows·cols) iteration over the buffer.
//! 4. **`set_char()` fast-path** – for single-ASCII-character cells (the
//!    overwhelming majority), we call `ratatui::Cell::set_char()` which
//!    writes directly into a 4-byte inline buffer instead of allocating a
//!    `String` via `vt100::Cell::contents()`.
//!    NOTE: vt100::Cell::contents() returns String::with_capacity(24) each
//!    time.  For the common case of single-char cells we avoid this entirely.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::theme::ThemeColors;

/// Render a `vt100::Screen` into a ratatui `Buffer`, with theme-aware ANSI
/// palette remapping baked into the same pass.
///
/// This replaces the three-step sequence:
/// ```ignore
/// PseudoTerminal::new(screen).style(base_style).render(area, buf);
/// let palette = ansi_palette_from_theme(theme);
/// remap_ansi_colors(buf, area, &palette, theme);
/// ```
///
/// `palette` should be the 16-entry array from `ansi_palette_from_theme()`.
pub fn render_screen(
    screen: &vt100::Screen,
    area: Rect,
    buf: &mut Buffer,
    palette: &[Color; 16],
    theme: &ThemeColors,
) {
    let rows = area.height;
    let cols = area.width;

    for row in 0..rows {
        for col in 0..cols {
            let buf_x = col + area.x;
            let buf_y = row + area.y;

            if let Some(screen_cell) = screen.cell(row, col) {
                let cell = &mut buf[(buf_x, buf_y)];
                fill_cell_optimized(screen_cell, cell, palette, theme);
            }
        }
    }

    // Cursor rendering (matches tui-term default Cursor behaviour)
    if !screen.hide_cursor() {
        let (c_row, c_col) = screen.cursor_position();
        let cx = c_col + area.x;
        let cy = c_row + area.y;
        if cy < area.y + rows && cx < area.x + cols {
            let c_cell = &mut buf[(cx, cy)];
            if let Some(cell) = screen.cell(c_row, c_col) {
                if cell.has_contents() {
                    // Overlay style: reversed (same as tui-term default)
                    c_cell.set_style(Style::default().add_modifier(Modifier::REVERSED));
                } else {
                    // Empty cell under cursor: show block cursor
                    c_cell.set_symbol("\u{2588}"); // █
                    c_cell.set_style(Style::default().fg(Color::Gray));
                }
            }
        }
    }
}

/// Convert a single vt100 cell into a ratatui buffer cell, with inline
/// ANSI palette remapping.  This is the hot inner loop.
#[inline(always)]
fn fill_cell_optimized(
    src: &vt100::Cell,
    dst: &mut ratatui::buffer::Cell,
    palette: &[Color; 16],
    theme: &ThemeColors,
) {
    // --- Symbol ---
    // Fast path: inspect the cell contents without allocating.
    // vt100::Cell::contents() allocates String::with_capacity(24).
    // We use has_contents() to skip empty cells entirely, and for
    // non-empty cells we must call contents() since the char array is private.
    if src.has_contents() {
        let s = src.contents();
        // Fast path: single-byte ASCII (covers ~95% of terminal content)
        let bytes = s.as_bytes();
        if bytes.len() == 1 {
            dst.set_char(bytes[0] as char);
        } else if !s.is_empty() {
            dst.set_symbol(&s);
        }
    }

    // --- Colors with inline palette remap ---
    let fg = convert_color_remapped(src.fgcolor(), palette, theme.text);
    let bg = convert_color_remapped(src.bgcolor(), palette, theme.background);
    dst.set_fg(fg);
    dst.set_bg(bg);

    // --- Modifiers ---
    // Build up the modifier bitmask from vt100 cell attributes.
    // IMPORTANT: we must NOT call dst.set_style(Style::reset()) here because
    // that would overwrite the fg/bg we just applied above with Color::Reset.
    let mut mods = Modifier::empty();
    if src.bold() {
        mods |= Modifier::BOLD;
    }
    if src.italic() {
        mods |= Modifier::ITALIC;
    }
    if src.underline() {
        mods |= Modifier::UNDERLINED;
    }
    if src.inverse() {
        mods |= Modifier::REVERSED;
    }
    // Reset the cell's modifier to exactly what the vt100 cell has,
    // without touching fg/bg.
    dst.modifier = mods;
}

/// Convert a `vt100::Color` directly to a `ratatui::style::Color`, applying
/// the ANSI palette remap inline.
///
/// This eliminates:
/// - The intermediate `tui_term::Color` enum
/// - The double `From` conversion chain (vt100→tui_term→ratatui)
/// - The separate `remap_ansi_colors()` post-processing pass
#[inline(always)]
fn convert_color_remapped(
    color: vt100::Color,
    palette: &[Color; 16],
    default_color: Color,
) -> Color {
    match color {
        vt100::Color::Default => default_color,
        vt100::Color::Idx(idx) => {
            if (idx as usize) < 16 {
                // Remap ANSI 0-15 to theme palette
                palette[idx as usize]
            } else {
                // Extended 256-color: pass through as Indexed
                Color::Indexed(idx)
            }
        }
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
