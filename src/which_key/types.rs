use crossterm::event::{KeyCode, KeyModifiers};

use crate::command_palette::CommandAction;
use crate::config;
use crate::vim_mode::VimMode;

use super::KeyCombo;

// ── Runtime owned keybinding types ─────────────────────────────────────

/// An owned keybinding node, built at startup from config.
#[derive(Debug, Clone)]
pub struct RuntimeKeyBinding {
    pub key: KeyCombo,
    pub label: String,
    pub action: Option<CommandAction>,
    pub modes: Vec<VimMode>,
    pub children: Vec<RuntimeKeyBinding>,
    /// Section grouping for cheatsheet display. None = auto-group by mode/modifier.
    pub context: Option<String>,
}

impl RuntimeKeyBinding {
    pub fn active_in(&self, mode: VimMode) -> bool {
        self.modes.is_empty() || self.modes.contains(&mode)
    }

    #[allow(dead_code)]
    pub fn has_modifier(&self) -> bool {
        !self.key.modifiers.is_empty()
    }
}

// ── Binding match enum ─────────────────────────────────────────────────

pub enum BindingMatch {
    Leaf(CommandAction),
    Prefix(String, Vec<RuntimeKeyBinding>),
    None,
}

// ── Helper to parse a config string into a KeyCombo ────────────────────

pub(crate) fn parse(s: &str) -> KeyCombo {
    config::parse_key_combo(s).unwrap_or_else(|e| {
        tracing::error!("Bad keybinding config '{}': {}", s, e);
        // Fallback to something that won't match anything useful
        KeyCombo::new(KeyModifiers::NONE, KeyCode::F(255))
    })
}

// ── Mode constants ─────────────────────────────────────────────────────

pub(crate) const ALL_MODES: &[VimMode] = &[];
pub(crate) const NORMAL_MODES: &[VimMode] = &[VimMode::Normal];
pub(crate) const RESIZE_MODES: &[VimMode] = &[VimMode::Resize];
pub(crate) const INSERT_MODES: &[VimMode] = &[VimMode::Insert];
pub(crate) const NORMAL_INSERT_MODES: &[VimMode] = &[VimMode::Normal, VimMode::Insert];

// ── Builder helpers ────────────────────────────────────────────────────

/// Build a leaf RuntimeKeyBinding.
pub(crate) fn rk_leaf(
    key: KeyCombo,
    label: &str,
    action: CommandAction,
    modes: &[VimMode],
) -> RuntimeKeyBinding {
    RuntimeKeyBinding {
        key,
        label: label.to_string(),
        action: Some(action),
        modes: modes.to_vec(),
        children: Vec::new(),
        context: None,
    }
}

/// Build a prefix RuntimeKeyBinding (has children, no direct action).
pub(crate) fn rk_prefix(
    key: KeyCombo,
    label: &str,
    modes: &[VimMode],
    children: Vec<RuntimeKeyBinding>,
) -> RuntimeKeyBinding {
    RuntimeKeyBinding {
        key,
        label: label.to_string(),
        action: None,
        modes: modes.to_vec(),
        children,
        context: None,
    }
}

/// Build a display-only RuntimeKeyBinding (cheatsheet only, not dispatched).
pub(crate) fn rk_display(
    key: KeyCombo,
    label: &str,
    modes: &[VimMode],
    context: &str,
) -> RuntimeKeyBinding {
    RuntimeKeyBinding {
        key,
        label: label.to_string(),
        action: None,
        modes: modes.to_vec(),
        children: Vec::new(),
        context: Some(context.to_string()),
    }
}
