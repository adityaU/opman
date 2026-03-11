use crate::config::format_key_display;
use crate::config::KeyBindings;

use super::{leader_hint, leader_nested_hint, CommandAction, CommandEntry};

pub fn build_commands(keys: &KeyBindings) -> Vec<CommandEntry> {
    vec![
        CommandEntry {
            name: "Toggle Sidebar".into(),
            shorthand: "sidebar".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_window, &keys.window_sidebar),
            action: CommandAction::ToggleSidebar,
        },
        CommandEntry {
            name: "Toggle Integrated Terminal".into(),
            shorthand: "terminal".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_terminal, &keys.terminal_toggle),
            action: CommandAction::ToggleTerminal,
        },
        CommandEntry {
            name: "New Terminal Tab".into(),
            shorthand: "newtab".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_terminal, &keys.terminal_new_tab),
            action: CommandAction::NewTerminalTab,
        },
        CommandEntry {
            name: "Next Terminal Tab".into(),
            shorthand: "nexttab".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_terminal, &keys.terminal_next_tab),
            action: CommandAction::NextTerminalTab,
        },
        CommandEntry {
            name: "Previous Terminal Tab".into(),
            shorthand: "prevtab".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_terminal, &keys.terminal_prev_tab),
            action: CommandAction::PrevTerminalTab,
        },
        CommandEntry {
            name: "Close Terminal Tab".into(),
            shorthand: "closetab".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_terminal, &keys.terminal_close_tab),
            action: CommandAction::CloseTerminalTab,
        },
        CommandEntry {
            name: "Navigate Left".into(),
            shorthand: "left".into(),
            keys_hint: format_key_display(&keys.nav_left),
            action: CommandAction::NavigateLeft,
        },
        CommandEntry {
            name: "Navigate Right".into(),
            shorthand: "right".into(),
            keys_hint: format_key_display(&keys.nav_right),
            action: CommandAction::NavigateRight,
        },
        CommandEntry {
            name: "Navigate Up".into(),
            shorthand: "up".into(),
            keys_hint: format_key_display(&keys.nav_up),
            action: CommandAction::NavigateUp,
        },
        CommandEntry {
            name: "Navigate Down".into(),
            shorthand: "down".into(),
            keys_hint: format_key_display(&keys.nav_down),
            action: CommandAction::NavigateDown,
        },
        CommandEntry {
            name: "Swap Terminal Content".into(),
            shorthand: "swap terminal content".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_window, &keys.window_swap),
            action: CommandAction::SwapTerminal,
        },
        CommandEntry {
            name: "Swap with Sidebar".into(),
            shorthand: "swap sidebar".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_swap, &keys.swap_sidebar),
            action: CommandAction::SwapWithSidebar,
        },
        CommandEntry {
            name: "Swap with Opencode".into(),
            shorthand: "swap opencode".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_swap, &keys.swap_opencode),
            action: CommandAction::SwapWithOpencode,
        },
        CommandEntry {
            name: "Swap with Terminal".into(),
            shorthand: "swap terminal".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_swap, &keys.swap_terminal),
            action: CommandAction::SwapWithTerminal,
        },
        CommandEntry {
            name: "Swap with Neovim".into(),
            shorthand: "swap neovim".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_swap, &keys.swap_neovim),
            action: CommandAction::SwapWithNeovim,
        },
        CommandEntry {
            name: "Swap with Git Panel".into(),
            shorthand: "swap git".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_swap, &keys.swap_git),
            action: CommandAction::SwapWithGit,
        },
        CommandEntry {
            name: "Toggle Git Panel".into(),
            shorthand: "git".into(),
            keys_hint: leader_hint(keys, &keys.leader_git),
            action: CommandAction::ToggleGitPanel,
        },
        CommandEntry {
            name: "Project Picker".into(),
            shorthand: "projects".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_project, &keys.project_picker),
            action: CommandAction::FuzzyPicker,
        },
        CommandEntry {
            name: "Add Project".into(),
            shorthand: "add".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_project, &keys.project_add),
            action: CommandAction::AddProject,
        },
        CommandEntry {
            name: "Search Sessions".into(),
            shorthand: "search".into(),
            keys_hint: leader_hint(keys, &keys.leader_search),
            action: CommandAction::SearchSessions,
        },
        CommandEntry {
            name: "Quit".into(),
            shorthand: "quit".into(),
            keys_hint: leader_hint(keys, &keys.leader_quit),
            action: CommandAction::Quit,
        },
        CommandEntry {
            name: "Settings".into(),
            shorthand: "config".into(),
            keys_hint: leader_hint(keys, &keys.leader_config),
            action: CommandAction::ConfigPanel,
        },
        CommandEntry {
            name: "Session Selector".into(),
            shorthand: "sessions".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_project, &keys.project_sessions),
            action: CommandAction::SessionSelector,
        },
        CommandEntry {
            name: "Todo List".into(),
            shorthand: "todos".into(),
            keys_hint: leader_hint(keys, &keys.leader_todo),
            action: CommandAction::ToggleTodoPanel,
        },
        CommandEntry {
            name: "Zen Mode".into(),
            shorthand: "zen".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_zen, &keys.zen_toggle),
            action: CommandAction::ZenMode,
        },
        CommandEntry {
            name: "Zen Terminal".into(),
            shorthand: "zen terminal".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_zen, &keys.zen_terminal),
            action: CommandAction::ZenTerminal,
        },
        CommandEntry {
            name: "Zen Opencode".into(),
            shorthand: "zen opencode".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_zen, &keys.zen_opencode),
            action: CommandAction::ZenOpencode,
        },
        CommandEntry {
            name: "Zen Neovim".into(),
            shorthand: "zen neovim".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_zen, &keys.zen_neovim),
            action: CommandAction::ZenNeovim,
        },
        CommandEntry {
            name: "Zen Git".into(),
            shorthand: "zen git".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_zen, &keys.zen_git),
            action: CommandAction::ZenGit,
        },
        CommandEntry {
            name: "Search Terminal".into(),
            shorthand: "search terminal find".into(),
            keys_hint: "Ctrl+F".into(),
            action: CommandAction::SearchTerminal,
        },
        CommandEntry {
            name: "Search Next Match".into(),
            shorthand: "search next".into(),
            keys_hint: "Ctrl+N / Enter".into(),
            action: CommandAction::SearchNextMatch,
        },
        CommandEntry {
            name: "Search Prev Match".into(),
            shorthand: "search prev".into(),
            keys_hint: "Ctrl+P / Shift+Enter".into(),
            action: CommandAction::SearchPrevMatch,
        },
        CommandEntry {
            name: "Context Input".into(),
            shorthand: "context".into(),
            keys_hint: leader_hint(keys, &keys.leader_context),
            action: CommandAction::ContextInput,
        },
        CommandEntry {
            name: "Pop Out Panels".into(),
            shorthand: "popout float".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_window, &keys.window_popout),
            action: CommandAction::PopOutPanels,
        },
        CommandEntry {
            name: "Session Watcher".into(),
            shorthand: "watcher watch".into(),
            keys_hint: leader_hint(keys, "w"),
            action: CommandAction::SessionWatcher,
        },
        CommandEntry {
            name: "Slack Connect".into(),
            shorthand: "slack connect".into(),
            keys_hint: leader_hint(keys, &keys.leader_slack),
            action: CommandAction::SlackConnect,
        },
        CommandEntry {
            name: "Slack OAuth Login".into(),
            shorthand: "slack oauth login auth".into(),
            keys_hint: "".into(),
            action: CommandAction::SlackOAuth,
        },
        CommandEntry {
            name: "Slack Disconnect".into(),
            shorthand: "slack disconnect stop".into(),
            keys_hint: "".into(),
            action: CommandAction::SlackDisconnect,
        },
        CommandEntry {
            name: "Slack Status".into(),
            shorthand: "slack status info".into(),
            keys_hint: "".into(),
            action: CommandAction::SlackStatus,
        },
        CommandEntry {
            name: "Slack Logs".into(),
            shorthand: "slack logs events debug".into(),
            keys_hint: "".into(),
            action: CommandAction::SlackLogs,
        },
    ]
}
