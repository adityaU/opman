use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command_palette::CommandAction;
use crate::config::{self, KeyBindings};
use crate::vim_mode::VimMode;

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

// ── Helper to parse a config string into a KeyCombo ────────────────────

fn parse(s: &str) -> KeyCombo {
    config::parse_key_combo(s).unwrap_or_else(|e| {
        tracing::error!("Bad keybinding config '{}': {}", s, e);
        // Fallback to something that won't match anything useful
        KeyCombo::new(KeyModifiers::NONE, KeyCode::F(255))
    })
}

// ── Mode constants ─────────────────────────────────────────────────────

const ALL_MODES: &[VimMode] = &[];
const NORMAL_MODES: &[VimMode] = &[VimMode::Normal];
const RESIZE_MODES: &[VimMode] = &[VimMode::Resize];
const INSERT_MODES: &[VimMode] = &[VimMode::Insert];
const NORMAL_INSERT_MODES: &[VimMode] = &[VimMode::Normal, VimMode::Insert];

/// Build a leaf RuntimeKeyBinding.
fn rk_leaf(
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
fn rk_prefix(
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
fn rk_display(key: KeyCombo, label: &str, modes: &[VimMode], context: &str) -> RuntimeKeyBinding {
    RuntimeKeyBinding {
        key,
        label: label.to_string(),
        action: None,
        modes: modes.to_vec(),
        children: Vec::new(),
        context: Some(context.to_string()),
    }
}

// ── Build the full runtime keymap from config ──────────────────────────

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

/// Build the full global keymap tree from a `KeyBindings` config.
pub fn build_keymap(kb: &KeyBindings) -> Vec<RuntimeKeyBinding> {
    let space_children = build_space_children(kb);

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

// ── Binding match enum ─────────────────────────────────────────────────

pub enum BindingMatch {
    Leaf(CommandAction),
    Prefix(String, Vec<RuntimeKeyBinding>),
    None,
}

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

// ── Cheatsheet generation ──────────────────────────────────────────────

fn collect_bindings(
    bindings: &[RuntimeKeyBinding],
    mode: VimMode,
    prefix_str: &str,
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for b in bindings {
        if !b.active_in(mode) {
            continue;
        }
        let key_label = if prefix_str.is_empty() {
            format_key_label(&b.key)
        } else {
            format!("{}{}", prefix_str, format_key_label(&b.key))
        };
        if b.children.is_empty() {
            result.push((key_label, b.label.clone()));
        } else {
            let child_prefix = format!("{}+ ", key_label);
            result.extend(collect_bindings(&b.children, mode, &child_prefix));
        }
    }
    result
}

/// Generate cheatsheet sections from the runtime keymap.
pub fn generate_cheatsheet_sections(
    keymap: &[RuntimeKeyBinding],
) -> Vec<(String, Vec<(String, String)>)> {
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();

    // All Modes — bindings active everywhere (modes is empty, no context, no children)
    let all_mode: Vec<_> = keymap
        .iter()
        .filter(|b| b.modes.is_empty() && b.context.is_none() && b.children.is_empty())
        .map(|b| (format_key_label(&b.key), b.label.clone()))
        .collect();
    if !all_mode.is_empty() {
        sections.push(("All Modes".to_string(), all_mode));
    }

    // Normal Mode — non-modifier, non-prefix keys
    let mut normal = Vec::new();
    for b in keymap.iter() {
        if b.context.is_some() || !b.active_in(VimMode::Normal) || b.modes.is_empty() {
            continue;
        }
        if b.key.modifiers.is_empty() && b.children.is_empty() {
            normal.push((format_key_label(&b.key), b.label.clone()));
        }
    }
    if !normal.is_empty() {
        sections.push(("Normal Mode".to_string(), normal));
    }

    // Ctrl+ (Normal) — modifier keys in normal mode
    let mut ctrl = Vec::new();
    for b in keymap.iter() {
        if b.context.is_some() || !b.active_in(VimMode::Normal) || b.modes.is_empty() {
            continue;
        }
        if b.children.is_empty() && b.key.modifiers.contains(KeyModifiers::CONTROL) {
            ctrl.push((format_key_label(&b.key), b.label.clone()));
        }
    }
    if !ctrl.is_empty() {
        sections.push(("Ctrl+ (Normal)".to_string(), ctrl));
    }

    // Leader+ (Normal) — find the leader prefix binding and collect its children
    if let Some(leader) = keymap
        .iter()
        .find(|b| !b.children.is_empty() && b.context.is_none() && b.active_in(VimMode::Normal))
    {
        let space = collect_bindings(&leader.children, VimMode::Normal, "");
        if !space.is_empty() {
            let leader_label = format_key_label(&leader.key);
            sections.push((format!("{}+ (Normal)", leader_label), space));
        }
    }

    let mut insert = Vec::new();
    for b in keymap.iter() {
        if b.context.is_some() || b.modes.is_empty() {
            continue;
        }
        if b.active_in(VimMode::Insert) && b.children.is_empty() {
            insert.push((format_key_label(&b.key), b.label.clone()));
        }
    }
    if !insert.is_empty() {
        sections.push(("Insert Mode".to_string(), insert));
    }

    let resize: Vec<_> = keymap
        .iter()
        .filter(|b| b.context.is_none() && b.modes.contains(&VimMode::Resize))
        .map(|b| (format_key_label(&b.key), b.label.clone()))
        .collect();
    if !resize.is_empty() {
        sections.push(("Resize Mode".to_string(), resize));
    }

    // Context-grouped sections (Sidebar, Insert Mode, etc.)
    let mut context_map: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for b in keymap.iter() {
        if let Some(ref ctx) = b.context {
            let key_label = format_key_label(&b.key);
            if let Some(section) = context_map.iter_mut().find(|(name, _)| name == ctx) {
                section.1.push((key_label, b.label.clone()));
            } else {
                context_map.push((ctx.clone(), vec![(key_label, b.label.clone())]));
            }
        }
    }
    sections.extend(context_map);

    sections
}
