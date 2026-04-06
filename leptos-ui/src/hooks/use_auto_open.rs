//! Auto-open configuration — localStorage-persisted tool-call accordion defaults.
//!
//! Each tool category has its own toggle controlling whether its accordion
//! auto-expands when it appears. All toggles default to OFF. The config is
//! provided via Leptos context so any component can read it cheaply.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "opman_auto_open_config";

// ── Tool category enum ────────────────────────────────────────────

/// Every accordion-capable tool type gets its own auto-open key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolCategory {
    Bash,
    Subagent,
    Edit,
    Read,
    Write,
    TodoWrite,
    Other,
}

impl ToolCategory {
    pub const ALL: &[ToolCategory] = &[
        Self::Bash,
        Self::Subagent,
        Self::Edit,
        Self::Read,
        Self::Write,
        Self::TodoWrite,
        Self::Other,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Bash => "bash_output",
            Self::Subagent => "subagent_task",
            Self::Edit => "edit_tools",
            Self::Read => "read_tools",
            Self::Write => "write_tools",
            Self::TodoWrite => "todo_write",
            Self::Other => "other_tools",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Bash => "Bash Output",
            Self::Subagent => "Subagent Tasks",
            Self::Edit => "Edit Tools",
            Self::Read => "Read Tools",
            Self::Write => "Write Tools",
            Self::TodoWrite => "Todo List",
            Self::Other => "Other Tools",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Bash => "Shell, terminal, and bash command output panes",
            Self::Subagent => "Task / subagent session cards with nested messages",
            Self::Edit => "File edit tool accordions (edit, replace)",
            Self::Read => "File read tools (read, glob, grep, search)",
            Self::Write => "File write / create tool accordions",
            Self::TodoWrite => "Todo list accordion showing task progress",
            Self::Other => "All remaining tools not covered above",
        }
    }

    pub fn icon_path(self) -> &'static str {
        match self {
            Self::Bash => "M4 17l6-6-6-6 M12 19h8",
            Self::Subagent => "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M9 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8z M22 21v-2a4 4 0 0 0-3-3.87 M16 3.13a4 4 0 0 1 0 7.75",
            Self::Edit => "M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7 M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z",
            Self::Read => "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
            Self::Write => "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6 M16 13H8 M16 17H8 M10 9H8",
            Self::TodoWrite => "M9 11l3 3L22 4 M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11",
            Self::Other => "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z",
        }
    }

    /// Classify a tool name into its category.
    /// Returns `None` only for A2UI (truly inline, no accordion).
    pub fn classify(tool_name: &str) -> Option<Self> {
        if tool_name == "ui_render" || tool_name == "ui_ui_render" {
            return None; // inline — no accordion
        }
        if tool_name.contains("todowrite") || tool_name.contains("todo_write") {
            return Some(Self::TodoWrite);
        }
        if tool_name == "task" {
            return Some(Self::Subagent);
        }
        if tool_name.contains("bash")
            || tool_name.contains("shell")
            || tool_name.contains("terminal")
        {
            return Some(Self::Bash);
        }
        if tool_name.contains("edit") && !tool_name.contains("neovim") {
            return Some(Self::Edit);
        }
        if tool_name.contains("read")
            || tool_name.contains("glob")
            || tool_name.contains("grep")
            || tool_name.contains("search")
        {
            return Some(Self::Read);
        }
        if tool_name.contains("write") || tool_name.contains("create") {
            return Some(Self::Write);
        }
        Some(Self::Other)
    }
}

// ── Config type ────────────────────────────────────────────────────

/// Persisted auto-open preferences per tool category.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoOpenConfig {
    /// Bash / shell / terminal output pane.
    #[serde(default)]
    pub bash_output: bool,
    /// Task / subagent session card.
    #[serde(default)]
    pub subagent_task: bool,
    /// Edit / replace file tools.
    #[serde(default)]
    pub edit_tools: bool,
    /// Read / glob / grep / search tools.
    #[serde(default)]
    pub read_tools: bool,
    /// Write / create file tools.
    #[serde(default)]
    pub write_tools: bool,
    /// Todo list accordion.
    #[serde(default)]
    pub todo_write: bool,
    /// Everything else.
    #[serde(default)]
    pub other_tools: bool,
}

impl Default for AutoOpenConfig {
    fn default() -> Self {
        Self {
            bash_output: false,
            subagent_task: false,
            edit_tools: false,
            read_tools: false,
            write_tools: false,
            todo_write: false,
            other_tools: false,
        }
    }
}

impl AutoOpenConfig {
    /// Get the value for a category.
    pub fn get_category(&self, cat: ToolCategory) -> bool {
        match cat {
            ToolCategory::Bash => self.bash_output,
            ToolCategory::Subagent => self.subagent_task,
            ToolCategory::Edit => self.edit_tools,
            ToolCategory::Read => self.read_tools,
            ToolCategory::Write => self.write_tools,
            ToolCategory::TodoWrite => self.todo_write,
            ToolCategory::Other => self.other_tools,
        }
    }

    /// Toggle the value for a category.
    pub fn toggle_category(&mut self, cat: ToolCategory) {
        match cat {
            ToolCategory::Bash => self.bash_output = !self.bash_output,
            ToolCategory::Subagent => self.subagent_task = !self.subagent_task,
            ToolCategory::Edit => self.edit_tools = !self.edit_tools,
            ToolCategory::Read => self.read_tools = !self.read_tools,
            ToolCategory::Write => self.write_tools = !self.write_tools,
            ToolCategory::TodoWrite => self.todo_write = !self.todo_write,
            ToolCategory::Other => self.other_tools = !self.other_tools,
        }
    }
}

// ── Context wrapper ────────────────────────────────────────────────

/// Reactive context for auto-open config. Provide once at layout level.
#[derive(Clone, Copy)]
pub struct AutoOpenState {
    pub config: RwSignal<AutoOpenConfig>,
}

impl AutoOpenState {
    /// Read a snapshot without tracking.
    pub fn get(&self) -> AutoOpenConfig {
        self.config.get_untracked()
    }

    /// Toggle a category, persist immediately.
    pub fn toggle(&self, cat: ToolCategory) {
        self.config.update(|c| {
            c.toggle_category(cat);
            persist(c);
        });
    }

    /// Get category value without tracking.
    pub fn category(&self, cat: ToolCategory) -> bool {
        self.config.get_untracked().get_category(cat)
    }

    /// Reactive category read (tracks signal).
    pub fn category_tracked(&self, cat: ToolCategory) -> impl Fn() -> bool {
        let config = self.config;
        move || config.get().get_category(cat)
    }
}

// ── Persistence ────────────────────────────────────────────────────

fn persist(config: &AutoOpenConfig) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        if let Ok(json) = serde_json::to_string(config) {
            let _ = storage.set_item(STORAGE_KEY, &json);
        }
    }
}

fn load() -> AutoOpenConfig {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item(STORAGE_KEY).ok())
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

// ── Hook ───────────────────────────────────────────────────────────

/// Initialize auto-open state. Call once at layout level, provide via context.
pub fn use_auto_open() -> AutoOpenState {
    let config = RwSignal::new(load());
    AutoOpenState { config }
}
