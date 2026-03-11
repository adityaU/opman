use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct KeyCombo {
    pub modifiers: KeyModifiers,
    pub code: KeyCode,
}

impl KeyCombo {
    pub const fn new(modifiers: KeyModifiers, code: KeyCode) -> Self {
        Self { modifiers, code }
    }

    pub fn matches(&self, key: &KeyEvent) -> bool {
        self.code == key.code && key.modifiers == self.modifiers
    }
}

impl std::fmt::Debug for KeyCombo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeyCombo({})", format_key_label(self))
    }
}

pub fn format_key_label(combo: &KeyCombo) -> String {
    let has_ctrl = combo.modifiers.contains(KeyModifiers::CONTROL);
    let has_alt = combo.modifiers.contains(KeyModifiers::ALT);
    let has_shift = combo.modifiers.contains(KeyModifiers::SHIFT);

    let mut prefix = String::new();
    if has_ctrl {
        prefix.push_str("Ctrl+");
    }
    if has_alt {
        prefix.push_str("Alt+");
    }
    if has_shift {
        prefix.push_str("Shift+");
    }

    let key_part = match combo.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "Bksp".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::F(n) => format!("F{}", n),
        _ => "?".to_string(),
    };

    format!("{}{}", prefix, key_part)
}
