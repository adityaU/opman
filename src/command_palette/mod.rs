mod commands;
mod palette;

use crate::config::{format_key_display, KeyBindings};

pub use commands::build_commands;
pub use palette::CommandPalette;

/// Actions that can be triggered from the command palette or keybindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAction {
    ToggleSidebar,
    ToggleTerminal,
    NavigateLeft,
    NavigateRight,
    NavigateUp,
    NavigateDown,
    SwapTerminal,
    ToggleGitPanel,
    FuzzyPicker,
    AddProject,
    SearchSessions,
    ToggleNeovim,
    ZenMode,
    ZenTerminal,
    ZenOpencode,
    ZenNeovim,
    ZenGit,
    ConfigPanel,
    Quit,
    // Mode transitions
    InsertMode,
    CommandMode,
    ResizeMode,
    // Resize actions
    ResizeLeft,
    ResizeRight,
    ResizeUp,
    ResizeDown,
    // Extra
    ToggleCheatsheet,
    // Targeted panel swaps
    SwapWithSidebar,
    SwapWithOpencode,
    SwapWithTerminal,
    SwapWithNeovim,
    SwapWithGit,
    SessionSelector,
    ToggleTodoPanel,
    NewTerminalTab,
    NextTerminalTab,
    PrevTerminalTab,
    CloseTerminalTab,
    SearchTerminal,
    SearchNextMatch,
    SearchPrevMatch,
    ContextInput,
    PopOutPanels,
    SessionWatcher,
    SlackConnect,
    SlackOAuth,
    SlackDisconnect,
    SlackStatus,
    SlackLogs,
    ToggleRoutinePanel,
}

pub struct CommandEntry {
    pub name: String,
    pub shorthand: String,
    pub keys_hint: String,
    pub action: CommandAction,
}

pub(crate) fn leader_hint(keys: &KeyBindings, sub_key: &str) -> String {
    format!(
        "{}+{}",
        format_key_display(&keys.leader),
        format_key_display(sub_key),
    )
}

pub(crate) fn leader_nested_hint(keys: &KeyBindings, prefix: &str, sub_key: &str) -> String {
    format!(
        "{}+{}+{}",
        format_key_display(&keys.leader),
        format_key_display(prefix),
        format_key_display(sub_key),
    )
}
