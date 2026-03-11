use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
/// Handle keys when the terminal pane is focused.
///
/// Most keys are forwarded directly to the PTY child process.
pub(super) fn handle_terminal_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    // Forward the key to the active project's PTY
    if let Some(project) = app.active_project_mut() {
        if let Some(pty) = project.active_pty_mut() {
            if pty.scroll_offset > 0 {
                pty.scroll_offset = 0;
                if let Ok(mut parser) = pty.parser.lock() {
                    parser.set_scrollback(0);
                }
            }
            let bytes = key_event_to_bytes(&key);
            if !bytes.is_empty() {
                pty.write(&bytes)?;
            }
        }
    }

    Ok(())
}

/// Convert a crossterm `KeyEvent` to bytes suitable for writing to a PTY.
///
/// Full xterm-compatible implementation supporting all modifiers and key codes.
pub(super) fn key_event_to_bytes(key: &KeyEvent) -> Vec<u8> {
    let has_alt = key.modifiers.contains(KeyModifiers::ALT);
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);

    // Compute xterm modifier parameter: 1 + (shift?1:0) + (alt?2:0) + (ctrl?4:0)
    let modifier_param = 1
        + if has_shift { 1 } else { 0 }
        + if has_alt { 2 } else { 0 }
        + if has_ctrl { 4 } else { 0 };
    let has_modifiers = modifier_param > 1;

    match key.code {
        KeyCode::Char(c) => {
            if has_ctrl && !has_alt {
                // Ctrl+letter → control character (0x01-0x1A)
                let ctrl_byte = (c.to_ascii_lowercase() as u8)
                    .wrapping_sub(b'a')
                    .wrapping_add(1);
                vec![ctrl_byte]
            } else if has_ctrl && has_alt {
                // Ctrl+Alt+letter → ESC + control character
                let ctrl_byte = (c.to_ascii_lowercase() as u8)
                    .wrapping_sub(b'a')
                    .wrapping_add(1);
                vec![0x1b, ctrl_byte]
            } else if has_alt {
                // Alt+key → ESC prefix + character
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                let mut bytes = vec![0x1b];
                bytes.extend_from_slice(s.as_bytes());
                bytes
            } else {
                // Plain character (including Shift which is already reflected in `c`)
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => {
            if has_alt {
                vec![0x1b, b'\r']
            } else {
                vec![b'\r']
            }
        }
        KeyCode::Backspace => {
            if has_alt {
                vec![0x1b, 0x7f]
            } else {
                vec![0x7f]
            }
        }
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => vec![0x1b, b'[', b'Z'], // Shift+Tab
        KeyCode::Esc => vec![0x1b],

        // Arrow keys: plain=\e[X, with modifiers=\e[1;{mod}X
        KeyCode::Up => {
            if has_modifiers {
                format!("\x1b[1;{}A", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'A']
            }
        }
        KeyCode::Down => {
            if has_modifiers {
                format!("\x1b[1;{}B", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'B']
            }
        }
        KeyCode::Right => {
            if has_modifiers {
                format!("\x1b[1;{}C", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'C']
            }
        }
        KeyCode::Left => {
            if has_modifiers {
                format!("\x1b[1;{}D", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'D']
            }
        }

        // Home/End: plain=\e[H/\e[F, with modifiers=\e[1;{mod}H/F
        KeyCode::Home => {
            if has_modifiers {
                format!("\x1b[1;{}H", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'H']
            }
        }
        KeyCode::End => {
            if has_modifiers {
                format!("\x1b[1;{}F", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'F']
            }
        }

        // Tilde-style keys: \e[{code}~ or \e[{code};{mod}~
        KeyCode::Insert => {
            if has_modifiers {
                format!("\x1b[2;{}~", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'2', b'~']
            }
        }
        KeyCode::Delete => {
            if has_modifiers {
                format!("\x1b[3;{}~", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'3', b'~']
            }
        }
        KeyCode::PageUp => {
            if has_modifiers {
                format!("\x1b[5;{}~", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'5', b'~']
            }
        }
        KeyCode::PageDown => {
            if has_modifiers {
                format!("\x1b[6;{}~", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'6', b'~']
            }
        }

        // Function keys: F1-F4 use SS3 (plain) or CSI with modifiers
        // F5-F12 use tilde-style sequences
        KeyCode::F(n) => {
            let (code, letter) = match n {
                1 => (None, Some(b'P')), // \eOP or \e[1;{mod}P
                2 => (None, Some(b'Q')), // \eOQ
                3 => (None, Some(b'R')), // \eOR
                4 => (None, Some(b'S')), // \eOS
                5 => (Some(15), None),   // \e[15~
                6 => (Some(17), None),   // \e[17~
                7 => (Some(18), None),   // \e[18~
                8 => (Some(19), None),   // \e[19~
                9 => (Some(20), None),   // \e[20~
                10 => (Some(21), None),  // \e[21~
                11 => (Some(23), None),  // \e[23~
                12 => (Some(24), None),  // \e[24~
                _ => return Vec::new(),
            };
            match (code, letter) {
                (None, Some(l)) => {
                    if has_modifiers {
                        format!("\x1b[1;{}{}", modifier_param, l as char).into_bytes()
                    } else {
                        vec![0x1b, b'O', l]
                    }
                }
                (Some(c), None) => {
                    if has_modifiers {
                        format!("\x1b[{};{}~", c, modifier_param).into_bytes()
                    } else {
                        format!("\x1b[{}~", c).into_bytes()
                    }
                }
                _ => Vec::new(),
            }
        }

        _ => Vec::new(),
    }
}

/// Handle keys when the neovim pane is focused.
///
/// Forwards all keys to the neovim PTY.
pub(super) fn handle_neovim_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    if let Some(project) = app.active_project_mut() {
        if let Some(ref mut nvim_pty) = project
            .active_resources_mut()
            .and_then(|r| r.neovim_pty.as_mut())
        {
            if nvim_pty.scroll_offset > 0 {
                nvim_pty.scroll_offset = 0;
                if let Ok(mut parser) = nvim_pty.parser.lock() {
                    parser.set_scrollback(0);
                }
            }
            let bytes = key_event_to_bytes(&key);
            if !bytes.is_empty() {
                nvim_pty.write(&bytes)?;
            }
        }
    }
    Ok(())
}

/// Handle keys when the integrated terminal panel is focused.
///
/// Forwards all keys to the shell PTY, similar to terminal pane.
pub(super) fn handle_integrated_terminal_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    if let Some(project) = app.active_project_mut() {
        if let Some(shell_pty) = project.active_shell_pty_mut() {
            if shell_pty.scroll_offset > 0 {
                shell_pty.scroll_offset = 0;
                if let Ok(mut parser) = shell_pty.parser.lock() {
                    parser.set_scrollback(0);
                }
            }
            let bytes = key_event_to_bytes(&key);
            if !bytes.is_empty() {
                shell_pty.write(&bytes)?;
            }
        }
    }
    Ok(())
}

pub(super) fn handle_git_panel_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    if let Some(project) = app.active_project_mut() {
        if let Some(ref mut gitui_pty) = project.gitui_pty {
            if gitui_pty.scroll_offset > 0 {
                gitui_pty.scroll_offset = 0;
                if let Ok(mut parser) = gitui_pty.parser.lock() {
                    parser.set_scrollback(0);
                }
            }
            let bytes = key_event_to_bytes(&key);
            if !bytes.is_empty() {
                gitui_pty.write(&bytes)?;
            }
            return Ok(());
        }
    }
    Ok(())
}
