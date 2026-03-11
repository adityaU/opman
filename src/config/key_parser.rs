// ── Key string parser ───────────────────────────────────────────────────

use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};

use crate::which_key::KeyCombo;

/// Parse a human-readable key string into a `KeyCombo`.
///
/// Examples: `"ctrl+q"`, `"ctrl+shift+s"`, `"space"`, `"a"`, `"f1"`, `"ctrl+/"`.
pub fn parse_key_combo(s: &str) -> Result<KeyCombo> {
    let s = s.trim().to_lowercase();
    let parts: Vec<&str> = s.split('+').collect();

    let mut modifiers = KeyModifiers::NONE;
    let key_part;

    if parts.len() == 1 {
        key_part = parts[0];
    } else {
        // All parts except the last are modifiers
        for &m in &parts[..parts.len() - 1] {
            match m {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" | "meta" | "opt" | "option" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                other => anyhow::bail!("Unknown modifier '{other}' in key string '{s}'"),
            }
        }
        key_part = parts[parts.len() - 1];
    }

    let code = match key_part {
        "space" | "spc" => KeyCode::Char(' '),
        "enter" | "return" | "cr" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backspace" | "bs" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" | "pgdown" => KeyCode::PageDown,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        // Single character keys
        k if k.len() == 1 => {
            let ch = k.chars().next().unwrap();
            if modifiers.contains(KeyModifiers::SHIFT) && ch.is_ascii_alphabetic() {
                // Shift+letter: crossterm sends uppercase char with SHIFT modifier
                KeyCode::Char(ch.to_ascii_uppercase())
            } else {
                KeyCode::Char(ch)
            }
        }
        other => anyhow::bail!("Unknown key name '{other}' in key string '{s}'"),
    };

    Ok(KeyCombo::new(modifiers, code))
}

/// Format a key-combo string back to a display label, e.g. `"Ctrl+Q"`.
pub fn format_key_display(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let parts: Vec<&str> = s.split('+').collect();
    let mut out = Vec::new();
    for p in &parts {
        match *p {
            "ctrl" | "control" => out.push("Ctrl".to_string()),
            "alt" | "meta" | "opt" | "option" => out.push("Alt".to_string()),
            "shift" => out.push("Shift".to_string()),
            "space" | "spc" => out.push("Space".to_string()),
            "enter" | "return" | "cr" => out.push("Enter".to_string()),
            "esc" | "escape" => out.push("Esc".to_string()),
            "tab" => out.push("Tab".to_string()),
            "backspace" | "bs" => out.push("BS".to_string()),
            other => {
                if other.len() == 1 {
                    out.push(other.to_uppercase());
                } else {
                    // F-keys, special names
                    let mut c = other.chars();
                    let first = c.next().unwrap().to_uppercase().to_string();
                    out.push(format!("{first}{}", c.as_str()));
                }
            }
        }
    }
    out.join("+")
}
