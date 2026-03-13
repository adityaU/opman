// ── Default value functions for serde ───────────────────────────────────
//
// Each function provides the default value for the corresponding
// `KeyBindings` field when the user's config file omits it.
// These are referenced from serde `#[serde(default = "...")]` attributes
// via their full crate path: `crate::config::key_defaults::default_*`.

pub(crate) fn default_quit() -> String {
    "ctrl+q".into()
}
pub(crate) fn default_cheatsheet() -> String {
    "ctrl+/".into()
}
pub(crate) fn default_cheatsheet_alt() -> String {
    "?".into()
}
pub(crate) fn default_insert_i() -> String {
    "i".into()
}
pub(crate) fn default_insert_a() -> String {
    "a".into()
}
pub(crate) fn default_insert_enter() -> String {
    "enter".into()
}
pub(crate) fn default_command_mode() -> String {
    ":".into()
}
pub(crate) fn default_resize_mode() -> String {
    "r".into()
}
pub(crate) fn default_nav_left() -> String {
    "ctrl+h".into()
}
pub(crate) fn default_nav_right() -> String {
    "ctrl+l".into()
}
pub(crate) fn default_nav_down() -> String {
    "ctrl+j".into()
}
pub(crate) fn default_nav_up() -> String {
    "ctrl+k".into()
}
pub(crate) fn default_toggle_sidebar() -> String {
    "ctrl+b".into()
}
pub(crate) fn default_toggle_terminal() -> String {
    "ctrl+t".into()
}
pub(crate) fn default_toggle_neovim() -> String {
    "ctrl+n".into()
}
pub(crate) fn default_toggle_git() -> String {
    "ctrl+g".into()
}
pub(crate) fn default_leader_swap() -> String {
    "s".into()
}
pub(crate) fn default_swap_sidebar() -> String {
    "s".into()
}
pub(crate) fn default_swap_opencode() -> String {
    "o".into()
}
pub(crate) fn default_swap_terminal() -> String {
    "t".into()
}
pub(crate) fn default_swap_neovim() -> String {
    "n".into()
}
pub(crate) fn default_swap_git() -> String {
    "g".into()
}
pub(crate) fn default_leader() -> String {
    "space".into()
}
pub(crate) fn default_leader_terminal() -> String {
    "t".into()
}
pub(crate) fn default_leader_git() -> String {
    "g".into()
}
pub(crate) fn default_leader_neovim() -> String {
    "n".into()
}
pub(crate) fn default_leader_zen() -> String {
    "z".into()
}
pub(crate) fn default_zen_toggle() -> String {
    "z".into()
}
pub(crate) fn default_zen_terminal() -> String {
    "t".into()
}
pub(crate) fn default_zen_opencode() -> String {
    "o".into()
}
pub(crate) fn default_zen_neovim() -> String {
    "n".into()
}
pub(crate) fn default_zen_git() -> String {
    "g".into()
}
pub(crate) fn default_leader_config() -> String {
    "c".into()
}
pub(crate) fn default_leader_search() -> String {
    "/".into()
}
pub(crate) fn default_leader_quit() -> String {
    "q".into()
}
pub(crate) fn default_leader_todo() -> String {
    "d".into()
}
pub(crate) fn default_leader_context() -> String {
    "i".into()
}
pub(crate) fn default_leader_slack() -> String {
    "S".into()
}
pub(crate) fn default_leader_routine() -> String {
    "R".into()
}
pub(crate) fn default_leader_project() -> String {
    "p".into()
}
pub(crate) fn default_project_picker() -> String {
    "p".into()
}
pub(crate) fn default_project_add() -> String {
    "a".into()
}
pub(crate) fn default_project_sessions() -> String {
    "s".into()
}
pub(crate) fn default_leader_window() -> String {
    "w".into()
}
pub(crate) fn default_window_left() -> String {
    "h".into()
}
pub(crate) fn default_window_right() -> String {
    "l".into()
}
pub(crate) fn default_window_down() -> String {
    "j".into()
}
pub(crate) fn default_window_up() -> String {
    "k".into()
}
pub(crate) fn default_window_float() -> String {
    "f".into()
}
pub(crate) fn default_window_popout() -> String {
    "w".into()
}
pub(crate) fn default_window_swap() -> String {
    "s".into()
}
pub(crate) fn default_window_sidebar() -> String {
    "b".into()
}
pub(crate) fn default_resize_left() -> String {
    "h".into()
}
pub(crate) fn default_resize_right() -> String {
    "l".into()
}
pub(crate) fn default_resize_down() -> String {
    "j".into()
}
pub(crate) fn default_resize_up() -> String {
    "k".into()
}
pub(crate) fn default_terminal_toggle() -> String {
    "t".into()
}
pub(crate) fn default_terminal_new_tab() -> String {
    "n".into()
}
pub(crate) fn default_terminal_next_tab() -> String {
    "]".into()
}
pub(crate) fn default_terminal_prev_tab() -> String {
    "[".into()
}
pub(crate) fn default_terminal_close_tab() -> String {
    "x".into()
}
pub(crate) fn default_terminal_search() -> String {
    "f".into()
}
