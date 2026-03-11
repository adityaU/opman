// ── Keybinding configuration ────────────────────────────────────────────

use serde::{Deserialize, Serialize};

use super::key_defaults;

/// User-configurable keybindings.
///
/// Each field is a key-combo string such as `"ctrl+q"`, `"space"`, `"a"`, `"ctrl+shift+s"`.
/// Supported modifiers: `ctrl`, `alt`, `shift`. Separator: `+`.
/// Special key names: `space`, `enter`, `esc`, `tab`, `backspace`, `up`, `down`, `left`, `right`,
/// `home`, `end`, `pageup`, `pagedown`, `delete`, `insert`, `f1`..`f12`, `/`, `.`, `?`, `:`.
///
/// The "leader" key opens the which-key prefix menu. Sub-bindings under the leader
/// (e.g. `leader_terminal`) are single keys pressed *after* the leader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    // ── Global ──────────────────────────────────────────────────────
    #[serde(default = "crate::config::key_defaults::default_quit")]
    pub quit: String,
    #[serde(default = "crate::config::key_defaults::default_cheatsheet")]
    pub cheatsheet: String,
    #[serde(default = "crate::config::key_defaults::default_cheatsheet_alt")]
    pub cheatsheet_alt: String,
    #[serde(default = "crate::config::key_defaults::default_insert_i")]
    pub insert_i: String,
    #[serde(default = "crate::config::key_defaults::default_insert_a")]
    pub insert_a: String,
    #[serde(default = "crate::config::key_defaults::default_insert_enter")]
    pub insert_enter: String,
    #[serde(default = "crate::config::key_defaults::default_command_mode")]
    pub command_mode: String,
    #[serde(default = "crate::config::key_defaults::default_resize_mode")]
    pub resize_mode: String,

    // ── Navigation (normal + insert) ────────────────────────────────
    #[serde(default = "crate::config::key_defaults::default_nav_left")]
    pub nav_left: String,
    #[serde(default = "crate::config::key_defaults::default_nav_right")]
    pub nav_right: String,
    #[serde(default = "crate::config::key_defaults::default_nav_down")]
    pub nav_down: String,
    #[serde(default = "crate::config::key_defaults::default_nav_up")]
    pub nav_up: String,

    // ── Direct shortcuts ────────────────────────────────────────────
    #[serde(default = "crate::config::key_defaults::default_toggle_sidebar")]
    pub toggle_sidebar: String,
    #[serde(default = "crate::config::key_defaults::default_toggle_terminal")]
    pub toggle_terminal: String,
    #[serde(default = "crate::config::key_defaults::default_toggle_neovim")]
    pub toggle_neovim: String,
    #[serde(default = "crate::config::key_defaults::default_toggle_git")]
    pub toggle_git: String,

    // ── Leader → Swap sub-bindings ──────────────────────────────────
    #[serde(default = "crate::config::key_defaults::default_leader_swap")]
    pub leader_swap: String,
    #[serde(default = "crate::config::key_defaults::default_swap_sidebar")]
    pub swap_sidebar: String,
    #[serde(default = "crate::config::key_defaults::default_swap_opencode")]
    pub swap_opencode: String,
    #[serde(default = "crate::config::key_defaults::default_swap_terminal")]
    pub swap_terminal: String,
    #[serde(default = "crate::config::key_defaults::default_swap_neovim")]
    pub swap_neovim: String,
    #[serde(default = "crate::config::key_defaults::default_swap_git")]
    pub swap_git: String,
    // ── Leader key ──────────────────────────────────────────────────
    #[serde(default = "crate::config::key_defaults::default_leader")]
    pub leader: String,

    // ── Leader sub-bindings (pressed after leader) ──────────────────
    #[serde(default = "crate::config::key_defaults::default_leader_terminal")]
    pub leader_terminal: String,
    #[serde(default = "crate::config::key_defaults::default_leader_git")]
    pub leader_git: String,
    #[serde(default = "crate::config::key_defaults::default_leader_neovim")]
    pub leader_neovim: String,
    #[serde(default = "crate::config::key_defaults::default_leader_zen")]
    pub leader_zen: String,
    #[serde(default = "crate::config::key_defaults::default_zen_toggle")]
    pub zen_toggle: String,
    #[serde(default = "crate::config::key_defaults::default_zen_terminal")]
    pub zen_terminal: String,
    #[serde(default = "crate::config::key_defaults::default_zen_opencode")]
    pub zen_opencode: String,
    #[serde(default = "crate::config::key_defaults::default_zen_neovim")]
    pub zen_neovim: String,
    #[serde(default = "crate::config::key_defaults::default_zen_git")]
    pub zen_git: String,
    #[serde(default = "crate::config::key_defaults::default_leader_config")]
    pub leader_config: String,
    #[serde(default = "crate::config::key_defaults::default_leader_search")]
    pub leader_search: String,
    #[serde(default = "crate::config::key_defaults::default_leader_quit")]
    pub leader_quit: String,
    #[serde(default = "crate::config::key_defaults::default_leader_todo")]
    pub leader_todo: String,
    #[serde(default = "crate::config::key_defaults::default_leader_context")]
    pub leader_context: String,
    #[serde(default = "crate::config::key_defaults::default_leader_slack")]
    pub leader_slack: String,
    // ── Leader → Terminal sub-bindings ──────────────────────────────
    #[serde(default = "crate::config::key_defaults::default_terminal_toggle")]
    pub terminal_toggle: String,
    #[serde(default = "crate::config::key_defaults::default_terminal_new_tab")]
    pub terminal_new_tab: String,
    #[serde(default = "crate::config::key_defaults::default_terminal_next_tab")]
    pub terminal_next_tab: String,
    #[serde(default = "crate::config::key_defaults::default_terminal_prev_tab")]
    pub terminal_prev_tab: String,
    #[serde(default = "crate::config::key_defaults::default_terminal_close_tab")]
    pub terminal_close_tab: String,
    #[serde(default = "crate::config::key_defaults::default_terminal_search")]
    pub terminal_search: String,

    // ── Leader → Project sub-bindings ───────────────────────────────
    #[serde(default = "crate::config::key_defaults::default_leader_project")]
    pub leader_project: String,
    #[serde(default = "crate::config::key_defaults::default_project_picker")]
    pub project_picker: String,
    #[serde(default = "crate::config::key_defaults::default_project_add")]
    pub project_add: String,
    #[serde(default = "crate::config::key_defaults::default_project_sessions")]
    pub project_sessions: String,

    // ── Leader → Window sub-bindings ────────────────────────────────
    #[serde(default = "crate::config::key_defaults::default_leader_window")]
    pub leader_window: String,
    #[serde(default = "crate::config::key_defaults::default_window_left")]
    pub window_left: String,
    #[serde(default = "crate::config::key_defaults::default_window_right")]
    pub window_right: String,
    #[serde(default = "crate::config::key_defaults::default_window_down")]
    pub window_down: String,
    #[serde(default = "crate::config::key_defaults::default_window_up")]
    pub window_up: String,
    #[serde(default = "crate::config::key_defaults::default_window_float")]
    pub window_float: String,
    #[serde(default = "crate::config::key_defaults::default_window_popout")]
    pub window_popout: String,
    #[serde(default = "crate::config::key_defaults::default_window_swap")]
    pub window_swap: String,
    #[serde(default = "crate::config::key_defaults::default_window_sidebar")]
    pub window_sidebar: String,

    // ── Resize mode ─────────────────────────────────────────────────
    #[serde(default = "crate::config::key_defaults::default_resize_left")]
    pub resize_left: String,
    #[serde(default = "crate::config::key_defaults::default_resize_right")]
    pub resize_right: String,
    #[serde(default = "crate::config::key_defaults::default_resize_down")]
    pub resize_down: String,
    #[serde(default = "crate::config::key_defaults::default_resize_up")]
    pub resize_up: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            quit: key_defaults::default_quit(),
            cheatsheet: key_defaults::default_cheatsheet(),
            cheatsheet_alt: key_defaults::default_cheatsheet_alt(),
            insert_i: key_defaults::default_insert_i(),
            insert_a: key_defaults::default_insert_a(),
            insert_enter: key_defaults::default_insert_enter(),
            command_mode: key_defaults::default_command_mode(),
            resize_mode: key_defaults::default_resize_mode(),
            nav_left: key_defaults::default_nav_left(),
            nav_right: key_defaults::default_nav_right(),
            nav_down: key_defaults::default_nav_down(),
            nav_up: key_defaults::default_nav_up(),
            toggle_sidebar: key_defaults::default_toggle_sidebar(),
            toggle_terminal: key_defaults::default_toggle_terminal(),
            toggle_neovim: key_defaults::default_toggle_neovim(),
            toggle_git: key_defaults::default_toggle_git(),
            leader_swap: key_defaults::default_leader_swap(),
            swap_sidebar: key_defaults::default_swap_sidebar(),
            swap_opencode: key_defaults::default_swap_opencode(),
            swap_terminal: key_defaults::default_swap_terminal(),
            swap_neovim: key_defaults::default_swap_neovim(),
            swap_git: key_defaults::default_swap_git(),
            leader: key_defaults::default_leader(),
            leader_terminal: key_defaults::default_leader_terminal(),
            leader_git: key_defaults::default_leader_git(),
            leader_neovim: key_defaults::default_leader_neovim(),
            leader_zen: key_defaults::default_leader_zen(),
            zen_toggle: key_defaults::default_zen_toggle(),
            zen_terminal: key_defaults::default_zen_terminal(),
            zen_opencode: key_defaults::default_zen_opencode(),
            zen_neovim: key_defaults::default_zen_neovim(),
            zen_git: key_defaults::default_zen_git(),
            leader_config: key_defaults::default_leader_config(),
            leader_search: key_defaults::default_leader_search(),
            leader_quit: key_defaults::default_leader_quit(),
            leader_todo: key_defaults::default_leader_todo(),
            leader_context: key_defaults::default_leader_context(),
            leader_slack: key_defaults::default_leader_slack(),
            terminal_toggle: key_defaults::default_terminal_toggle(),
            terminal_new_tab: key_defaults::default_terminal_new_tab(),
            terminal_next_tab: key_defaults::default_terminal_next_tab(),
            terminal_prev_tab: key_defaults::default_terminal_prev_tab(),
            terminal_close_tab: key_defaults::default_terminal_close_tab(),
            terminal_search: key_defaults::default_terminal_search(),
            leader_project: key_defaults::default_leader_project(),
            project_picker: key_defaults::default_project_picker(),
            project_add: key_defaults::default_project_add(),
            project_sessions: key_defaults::default_project_sessions(),
            leader_window: key_defaults::default_leader_window(),
            window_left: key_defaults::default_window_left(),
            window_right: key_defaults::default_window_right(),
            window_down: key_defaults::default_window_down(),
            window_up: key_defaults::default_window_up(),
            window_float: key_defaults::default_window_float(),
            window_popout: key_defaults::default_window_popout(),
            window_swap: key_defaults::default_window_swap(),
            window_sidebar: key_defaults::default_window_sidebar(),
            resize_left: key_defaults::default_resize_left(),
            resize_right: key_defaults::default_resize_right(),
            resize_down: key_defaults::default_resize_down(),
            resize_up: key_defaults::default_resize_up(),
        }
    }
}
