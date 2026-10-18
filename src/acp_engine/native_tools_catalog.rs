//! The native tools each harness lets opman switch off, and nothing else.
//!
//! A harness appears here only if its way of dropping a tool was run against the real
//! agent. Every other harness has no entry, and the settings page says so rather than
//! showing switches that do nothing. Tools the harness has but cannot drop are left out on
//! purpose: Codex's `view_image` and subagent tools ignore every switch that was tried.

use super::native_tools::{Mechanism, ToolDef, ToolSet};

const fn tool(id: &'static str, label: &'static str, description: &'static str) -> ToolDef {
    ToolDef {
        id,
        label,
        description,
    }
}

/// Claude Code tool names, as `disallowedTools` spells them.
static CLAUDE: ToolSet = ToolSet {
    mechanism: Mechanism::ClaudeSessionMeta,
    tools: &[
        tool("Bash", "Shell", "Run shell commands."),
        tool("Read", "Read files", "Read files and images from disk."),
        tool("Glob", "Find files", "Find files that match a pattern."),
        tool("Grep", "Search file contents", "Search inside files."),
        tool("Edit", "Edit files", "Change part of an existing file."),
        tool(
            "Write",
            "Write files",
            "Create a file or replace its contents.",
        ),
        tool(
            "NotebookEdit",
            "Edit notebooks",
            "Change cells in a Jupyter notebook.",
        ),
        tool("WebFetch", "Fetch web pages", "Download and read a URL."),
        tool("WebSearch", "Web search", "Search the web."),
        tool("Task", "Subagents", "Start a helper agent for a sub-task."),
        tool(
            "Agent",
            "Agents",
            "Start a helper agent in newer Claude versions.",
        ),
        tool(
            "TaskOutput",
            "Task output",
            "Read a background task's output.",
        ),
        tool("TaskStop", "Stop tasks", "Stop a background task."),
        tool(
            "TodoWrite",
            "Todo list",
            "Keep a task list for the session.",
        ),
        tool("TaskCreate", "Create tasks", "Create a tracked task."),
        tool("TaskGet", "Read tasks", "Read a tracked task."),
        tool("TaskUpdate", "Update tasks", "Change a tracked task."),
        tool("TaskList", "List tasks", "List tracked tasks."),
        tool(
            "AskUserQuestion",
            "Ask questions",
            "Ask the user a question.",
        ),
        tool(
            "EnterPlanMode",
            "Enter plan mode",
            "Start planning a change.",
        ),
        tool(
            "ExitPlanMode",
            "Leave plan mode",
            "Finish planning a change.",
        ),
        tool(
            "EnterWorktree",
            "Enter worktree",
            "Create or enter a Git worktree.",
        ),
        tool("ExitWorktree", "Leave worktree", "Leave a Git worktree."),
    ],
};

/// opencode permission keys. `edit` also covers its write and patch tools.
static OPENCODE: ToolSet = ToolSet {
    mechanism: Mechanism::OpencodePermission,
    tools: &[
        tool("bash", "Shell", "Run shell commands."),
        tool("read", "Read files", "Read files from disk."),
        tool(
            "edit",
            "Edit and write files",
            "Change, create or patch files.",
        ),
        tool("glob", "Find files", "List files that match a pattern."),
        tool("grep", "Search file contents", "Search inside files."),
        tool("webfetch", "Fetch web pages", "Download and read a URL."),
        tool("websearch", "Web search", "Search the web."),
        tool("task", "Subagents", "Start a helper agent for a sub-task."),
        tool(
            "todowrite",
            "Todo list",
            "Keep a task list for the session.",
        ),
    ],
};

/// Codex config paths. Only the three that were seen to remove the tool are listed.
static CODEX: ToolSet = ToolSet {
    mechanism: Mechanism::CodexConfig,
    tools: &[
        tool("features.shell_tool", "Shell", "Run shell commands."),
        tool("web_search", "Web search", "Search the web."),
        tool(
            "features.image_generation",
            "Image generation",
            "Create images.",
        ),
    ],
};

/// The tools opman can switch off for the catalogue agent `id`, if it can for that agent.
pub fn tool_set_for(id: &str) -> Option<&'static ToolSet> {
    match id {
        "claude" => Some(&CLAUDE),
        "opencode" => Some(&OPENCODE),
        "codex" => Some(&CODEX),
        _ => None,
    }
}
