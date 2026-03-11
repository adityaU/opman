use crossterm::event::{KeyCode, KeyModifiers};

use super::key_combo::KeyCombo;
use super::types::{
    parse, rk_display, rk_leaf, rk_prefix, RuntimeKeyBinding, ALL_MODES, INSERT_MODES,
    NORMAL_INSERT_MODES, NORMAL_MODES, RESIZE_MODES,
};
use crate::command_palette::CommandAction;
use crate::config::KeyBindings;

/// Build the full global keymap tree from a `KeyBindings` config.
pub fn build_keymap(kb: &KeyBindings) -> Vec<RuntimeKeyBinding> {
    let space_children = super::build_space_children(kb);

    let mut keymap = vec![
        // Global (all modes)
        rk_leaf(parse(&kb.quit), "Quit", CommandAction::Quit, ALL_MODES),
        rk_leaf(
            parse(&kb.cheatsheet),
            "Toggle Cheatsheet",
            CommandAction::ToggleCheatsheet,
            ALL_MODES,
        ),
        // Normal mode
        rk_leaf(
            parse(&kb.insert_i),
            "Insert Mode",
            CommandAction::InsertMode,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.insert_a),
            "Insert Mode",
            CommandAction::InsertMode,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.insert_enter),
            "Insert Mode",
            CommandAction::InsertMode,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.command_mode),
            "Command Mode",
            CommandAction::CommandMode,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.resize_mode),
            "Resize Mode",
            CommandAction::ResizeMode,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.cheatsheet_alt),
            "Cheatsheet",
            CommandAction::ToggleCheatsheet,
            NORMAL_MODES,
        ),
        // Leader prefix
        rk_prefix(parse(&kb.leader), "Leader", NORMAL_MODES, space_children),
        // Direct shortcuts
        rk_leaf(
            parse(&kb.toggle_sidebar),
            "Toggle Sidebar",
            CommandAction::ToggleSidebar,
            NORMAL_MODES,
        ),
        // Navigation (normal + insert)
        rk_leaf(
            parse(&kb.nav_left),
            "Navigate Left",
            CommandAction::NavigateLeft,
            NORMAL_INSERT_MODES,
        ),
        rk_leaf(
            parse(&kb.nav_right),
            "Navigate Right",
            CommandAction::NavigateRight,
            NORMAL_INSERT_MODES,
        ),
        rk_leaf(
            parse(&kb.nav_down),
            "Navigate Down",
            CommandAction::NavigateDown,
            NORMAL_INSERT_MODES,
        ),
        rk_leaf(
            parse(&kb.nav_up),
            "Navigate Up",
            CommandAction::NavigateUp,
            NORMAL_INSERT_MODES,
        ),
        rk_leaf(
            parse(&kb.toggle_terminal),
            "Toggle Terminal",
            CommandAction::ToggleTerminal,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.toggle_neovim),
            "Toggle Neovim",
            CommandAction::ToggleNeovim,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.toggle_git),
            "Toggle Git Panel",
            CommandAction::ToggleGitPanel,
            NORMAL_MODES,
        ),
        // Resize mode keys
        rk_leaf(
            parse(&kb.resize_left),
            "Resize Left",
            CommandAction::ResizeLeft,
            RESIZE_MODES,
        ),
        rk_leaf(
            parse(&kb.resize_right),
            "Resize Right",
            CommandAction::ResizeRight,
            RESIZE_MODES,
        ),
        rk_leaf(
            parse(&kb.resize_down),
            "Resize Down",
            CommandAction::ResizeDown,
            RESIZE_MODES,
        ),
        rk_leaf(
            parse(&kb.resize_up),
            "Resize Up",
            CommandAction::ResizeUp,
            RESIZE_MODES,
        ),
        // Navigation also works in resize mode
        rk_leaf(
            parse(&kb.nav_left),
            "Navigate Left",
            CommandAction::NavigateLeft,
            RESIZE_MODES,
        ),
        rk_leaf(
            parse(&kb.nav_right),
            "Navigate Right",
            CommandAction::NavigateRight,
            RESIZE_MODES,
        ),
        rk_leaf(
            parse(&kb.nav_down),
            "Navigate Down",
            CommandAction::NavigateDown,
            RESIZE_MODES,
        ),
        rk_leaf(
            parse(&kb.nav_up),
            "Navigate Up",
            CommandAction::NavigateUp,
            RESIZE_MODES,
        ),
    ];

    // Display-only bindings (shown in cheatsheet, not dispatched through registry)
    let display_bindings = vec![
        // Sidebar
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Char('j')),
            "Move down",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Char('k')),
            "Move up",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Down),
            "Move down",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Up),
            "Move up",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Enter),
            "Select / expand",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Char('a')),
            "Add project",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Char('o')),
            "Toggle subagents",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Char('d')),
            "Delete project",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Char('?')),
            "Toggle cheatsheet",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Char('q')),
            "Quit",
            NORMAL_MODES,
            "Sidebar",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Esc),
            "Normal mode",
            NORMAL_MODES,
            "Sidebar",
        ),
        // Insert Mode
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Esc),
            "Forward Esc to PTY",
            INSERT_MODES,
            "Insert Mode",
        ),
        rk_display(
            KeyCombo::new(KeyModifiers::NONE, KeyCode::Esc),
            "Double-Esc -> Normal mode",
            INSERT_MODES,
            "Insert Mode",
        ),
    ];

    keymap.extend(display_bindings);
    keymap
}
