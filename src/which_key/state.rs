use crossterm::event::KeyEvent;

use crate::command_palette::CommandAction;
use crate::vim_mode::VimMode;

use super::key_combo::format_key_label;
use super::types::{BindingMatch, RuntimeKeyBinding};

/// Look up a key event in the runtime keymap, returning the matching action or prefix.
pub fn lookup_binding(key: &KeyEvent, mode: VimMode, keymap: &[RuntimeKeyBinding]) -> BindingMatch {
    for binding in keymap {
        if binding.key.matches(key) && binding.active_in(mode) {
            if let Some(action) = binding.action {
                return BindingMatch::Leaf(action);
            }
            if !binding.children.is_empty() {
                let label = format_key_label(&binding.key);
                return BindingMatch::Prefix(label, binding.children.clone());
            }
        }
    }
    BindingMatch::None
}

// ── WhichKeyState ──────────────────────────────────────────────────────

pub struct WhichKeyState {
    pub active: bool,
    pub keymap: Vec<RuntimeKeyBinding>,
    pub pending_prefix_label: String,
    /// The default leader children, used to reset on deactivate.
    default_children: Vec<RuntimeKeyBinding>,
}

impl WhichKeyState {
    /// Create a new WhichKeyState. The default_children should be the space
    /// (leader) children built from config.
    pub fn new(default_children: Vec<RuntimeKeyBinding>) -> Self {
        let keymap = default_children.clone();
        Self {
            active: false,
            keymap,
            pending_prefix_label: String::new(),
            default_children,
        }
    }

    pub fn activate_with(&mut self, prefix_label: String, bindings: Vec<RuntimeKeyBinding>) {
        self.active = true;
        self.keymap = bindings;
        self.pending_prefix_label = prefix_label;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.keymap = self.default_children.clone();
        self.pending_prefix_label.clear();
    }

    pub fn process_key(&mut self, key: &KeyEvent) -> Option<CommandAction> {
        // Clone the keymap to avoid borrow issues during iteration
        let current = self.keymap.clone();
        for binding in &current {
            if binding.key.matches(key) {
                if let Some(action) = binding.action {
                    self.deactivate();
                    return Some(action);
                }
                if !binding.children.is_empty() {
                    let label = format!(
                        "{} {}",
                        self.pending_prefix_label,
                        format_key_label(&binding.key)
                    );
                    self.pending_prefix_label = label;
                    self.keymap = binding.children.clone();
                    return None;
                }
            }
        }
        self.deactivate();
        None
    }

    pub fn current_bindings(&self) -> &[RuntimeKeyBinding] {
        &self.keymap
    }
}
