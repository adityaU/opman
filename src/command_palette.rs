use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};

use crate::config::{format_key_display, KeyBindings};

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
    SwapPanel,
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
}

pub struct CommandEntry {
    pub name: String,
    pub shorthand: String,
    pub keys_hint: String,
    pub action: CommandAction,
}

fn leader_hint(keys: &KeyBindings, sub_key: &str) -> String {
    format!(
        "{}+{}",
        format_key_display(&keys.leader),
        format_key_display(sub_key),
    )
}

fn leader_nested_hint(keys: &KeyBindings, prefix: &str, sub_key: &str) -> String {
    format!(
        "{}+{}+{}",
        format_key_display(&keys.leader),
        format_key_display(prefix),
        format_key_display(sub_key),
    )
}

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
            name: "Swap Terminal".into(),
            shorthand: "swap".into(),
            keys_hint: leader_nested_hint(keys, &keys.leader_window, &keys.window_swap),
            action: CommandAction::SwapTerminal,
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
    ]
}

pub struct CommandPalette {
    pub query: String,
    pub cursor_pos: usize,
    pub selected: usize,
    pub scroll_offset: usize,
    matcher: Matcher,
    filtered: Vec<(usize, u32)>,
    commands: Vec<CommandEntry>,
}

impl CommandPalette {
    pub fn new(keys: &KeyBindings) -> Self {
        Self {
            query: String::new(),
            cursor_pos: 0,
            selected: 0,
            scroll_offset: 0,
            matcher: Matcher::new(Config::DEFAULT),
            filtered: Vec::new(),
            commands: build_commands(keys),
        }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.cursor_pos = 0;
        self.selected = 0;
        self.scroll_offset = 0;
        self.filtered.clear();
    }

    pub fn tick(&mut self) {
        self.filtered.clear();
        if self.query.is_empty() {
            for (i, _) in self.commands.iter().enumerate() {
                self.filtered.push((i, 0));
            }
        } else {
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Smart,
                Normalization::Smart,
                nucleo::pattern::AtomKind::Fuzzy,
            );
            for (i, cmd) in self.commands.iter().enumerate() {
                let haystack = format!("{} {}", cmd.name, cmd.shorthand);
                if let Some(score) = pattern.score(
                    nucleo::Utf32Str::Ascii(haystack.as_bytes()),
                    &mut self.matcher,
                ) {
                    self.filtered.push((i, score));
                }
            }
            self.filtered.sort_by(|a, b| b.1.cmp(&a.1));
        }
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn filtered_commands(&self) -> Vec<&CommandEntry> {
        self.filtered
            .iter()
            .map(|(i, _)| &self.commands[*i])
            .collect()
    }

    pub fn selected_action(&self) -> Option<CommandAction> {
        self.filtered
            .get(self.selected)
            .map(|(i, _)| self.commands[*i].action)
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered.len() - 1);
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.query.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.query[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.query.replace_range(prev..self.cursor_pos, "");
            self.cursor_pos = prev;
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.query[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.query.len() {
            self.cursor_pos = self.query[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.query.len());
        }
    }
}
