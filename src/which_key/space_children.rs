use crate::command_palette::CommandAction;
use crate::config::KeyBindings;

use super::types::{parse, rk_leaf, rk_prefix, RuntimeKeyBinding, NORMAL_MODES};

/// Build the Space (leader) children sub-keymap from config.
pub fn build_space_children(kb: &KeyBindings) -> Vec<RuntimeKeyBinding> {
    let project_children = vec![
        rk_leaf(
            parse(&kb.project_picker),
            "Picker",
            CommandAction::FuzzyPicker,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.project_add),
            "Add",
            CommandAction::AddProject,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.project_sessions),
            "Sessions",
            CommandAction::SessionSelector,
            NORMAL_MODES,
        ),
    ];

    let window_children = vec![
        rk_leaf(
            parse(&kb.window_left),
            "Navigate Left",
            CommandAction::NavigateLeft,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.window_right),
            "Navigate Right",
            CommandAction::NavigateRight,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.window_down),
            "Navigate Down",
            CommandAction::NavigateDown,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.window_up),
            "Navigate Up",
            CommandAction::NavigateUp,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.window_swap),
            "Swap",
            CommandAction::SwapTerminal,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.window_sidebar),
            "Sidebar",
            CommandAction::ToggleSidebar,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.window_popout),
            "Pop Out",
            CommandAction::PopOutPanels,
            NORMAL_MODES,
        ),
    ];

    vec![
        rk_prefix(
            parse(&kb.leader_project),
            "Project",
            NORMAL_MODES,
            project_children,
        ),
        rk_prefix(
            parse(&kb.leader_window),
            "Window",
            NORMAL_MODES,
            window_children,
        ),
        {
            let terminal_children = vec![
                rk_leaf(
                    parse(&kb.terminal_toggle),
                    "Toggle Terminal",
                    CommandAction::ToggleTerminal,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.terminal_new_tab),
                    "New Tab",
                    CommandAction::NewTerminalTab,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.terminal_next_tab),
                    "Next Tab",
                    CommandAction::NextTerminalTab,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.terminal_prev_tab),
                    "Prev Tab",
                    CommandAction::PrevTerminalTab,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.terminal_close_tab),
                    "Close Tab",
                    CommandAction::CloseTerminalTab,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.terminal_search),
                    "Search",
                    CommandAction::SearchTerminal,
                    NORMAL_MODES,
                ),
            ];
            rk_prefix(
                parse(&kb.leader_terminal),
                "Terminal",
                NORMAL_MODES,
                terminal_children,
            )
        },
        rk_leaf(
            parse(&kb.leader_git),
            "Git Panel",
            CommandAction::ToggleGitPanel,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.leader_neovim),
            "Neovim",
            CommandAction::ToggleNeovim,
            NORMAL_MODES,
        ),
        {
            let zen_children = vec![
                rk_leaf(
                    parse(&kb.zen_toggle),
                    "Toggle Zen",
                    CommandAction::ZenMode,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.zen_terminal),
                    "Zen Terminal",
                    CommandAction::ZenTerminal,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.zen_opencode),
                    "Zen Opencode",
                    CommandAction::ZenOpencode,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.zen_neovim),
                    "Zen Neovim",
                    CommandAction::ZenNeovim,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.zen_git),
                    "Zen Git",
                    CommandAction::ZenGit,
                    NORMAL_MODES,
                ),
            ];
            rk_prefix(parse(&kb.leader_zen), "Zen", NORMAL_MODES, zen_children)
        },
        rk_leaf(
            parse(&kb.leader_config),
            "Settings",
            CommandAction::ConfigPanel,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.leader_search),
            "Search Sessions",
            CommandAction::SearchSessions,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.leader_quit),
            "Quit",
            CommandAction::Quit,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.leader_todo),
            "Todo List",
            CommandAction::ToggleTodoPanel,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.leader_context),
            "Context Input",
            CommandAction::ContextInput,
            NORMAL_MODES,
        ),
        rk_leaf(
            parse(&kb.leader_slack),
            "Slack",
            CommandAction::SlackConnect,
            NORMAL_MODES,
        ),
        {
            let swap_children = vec![
                rk_leaf(
                    parse(&kb.swap_sidebar),
                    "Sidebar",
                    CommandAction::SwapWithSidebar,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.swap_opencode),
                    "Opencode",
                    CommandAction::SwapWithOpencode,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.swap_terminal),
                    "Terminal",
                    CommandAction::SwapWithTerminal,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.swap_neovim),
                    "Neovim",
                    CommandAction::SwapWithNeovim,
                    NORMAL_MODES,
                ),
                rk_leaf(
                    parse(&kb.swap_git),
                    "Git",
                    CommandAction::SwapWithGit,
                    NORMAL_MODES,
                ),
            ];
            rk_prefix(parse(&kb.leader_swap), "Swap", NORMAL_MODES, swap_children)
        },
    ]
}
