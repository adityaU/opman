use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

/// Convert a crossterm `MouseEvent` to SGR (1006) encoded bytes for the PTY.
///
/// SGR encoding: `ESC [ < Cb ; Cx ; Cy M` (press) or `ESC [ < Cb ; Cx ; Cy m` (release)
pub fn mouse_event_to_bytes(event: &MouseEvent, panel_x: u16, panel_y: u16) -> Option<Vec<u8>> {
    let col = event.column.saturating_sub(panel_x) + 1;
    let row = event.row.saturating_sub(panel_y) + 1;

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            Some(format!("\x1b[<0;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Down(MouseButton::Right) => {
            Some(format!("\x1b[<2;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            Some(format!("\x1b[<1;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Up(MouseButton::Left) => {
            Some(format!("\x1b[<0;{};{}m", col, row).into_bytes())
        }
        MouseEventKind::Up(MouseButton::Right) => {
            Some(format!("\x1b[<2;{};{}m", col, row).into_bytes())
        }
        MouseEventKind::Up(MouseButton::Middle) => {
            Some(format!("\x1b[<1;{};{}m", col, row).into_bytes())
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            Some(format!("\x1b[<32;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Drag(MouseButton::Right) => {
            Some(format!("\x1b[<34;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Drag(MouseButton::Middle) => {
            Some(format!("\x1b[<33;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::ScrollUp => Some(format!("\x1b[<64;{};{}M", col, row).into_bytes()),
        MouseEventKind::ScrollDown => Some(format!("\x1b[<65;{};{}M", col, row).into_bytes()),
        MouseEventKind::ScrollLeft => Some(format!("\x1b[<66;{};{}M", col, row).into_bytes()),
        MouseEventKind::ScrollRight => Some(format!("\x1b[<67;{};{}M", col, row).into_bytes()),
        MouseEventKind::Moved => Some(format!("\x1b[<35;{};{}M", col, row).into_bytes()),
    }
}
