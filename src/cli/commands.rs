use crate::self_update::Version;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Run the MCP stdio bridge for a project
    Mcp {
        /// Path to the project directory
        project_path: Option<PathBuf>,
    },

    /// Front an OAuth-protected remote MCP server over stdio.
    ///
    /// Not for direct use: opman substitutes this for a remote server whenever the target
    /// runner cannot dial the endpoint, or the credential must be minted per request.
    McpProxy {
        /// Registry name of the server to front.
        name: String,
    },

    /// Run the skills MCP bridge (exposes `~/.config/opman/skills` to any runner)
    McpSkills,

    /// Run the time MCP bridge
    McpTime,

    /// Run the UI render MCP bridge (A2UI)
    McpUi,

    /// Run the Kanban MCP bridge (task lane/notes updates for launched tasks)
    McpKanban,

    /// Run the browser MCP bridge (drives the workspace's browser panes)
    McpBrowser {
        /// Path to the project directory — its browser is the one the tools default to
        project_path: Option<PathBuf>,
    },

    /// Run the code MCP bridge (one `execute` tool that scripts the other servers)
    McpCode {
        /// Path to the project directory
        project_path: Option<PathBuf>,
    },

    /// Run the ask MCP bridge (asks the user a structured question and waits)
    McpAsk {
        /// Path to the project directory
        project_path: Option<PathBuf>,
    },

    /// Run the coding-tools MCP server (bash, files, search, web fetch) for a project
    McpTools {
        /// Path to the project directory; relative paths start here
        project_path: Option<PathBuf>,
    },

    /// Run the agent-manager MCP bridge for a project
    McpAgentManager {
        /// Path to the project directory
        project_path: Option<PathBuf>,
    },

    /// Internal: PreToolUse permission/question hook relay for the Claude engine.
    /// Reads the hook payload on stdin, asks the running opman (via
    /// `OPMAN_ENGINE_URL`), and prints the permission decision. Not for direct use.
    ClaudeHook,

    /// Run the neovim MCP bridge for a project
    McpNvim {
        /// Path to the project directory
        project_path: Option<PathBuf>,
    },

    /// Print the Slack app manifest and copy to clipboard
    SlackManifest,

    /// Update opman to the latest release
    Update {
        /// Install this exact version instead of the latest (e.g. 0.10.1)
        #[arg(long, value_name = "VERSION")]
        version: Option<Version>,
    },

    /// Install an older release. Without a version, lists the older releases.
    Downgrade {
        /// The version to go back to (e.g. 0.10.1)
        version: Option<Version>,
    },

    /// Check this opman setup (ACP agents, MCP servers, tools, log) and print a report
    Doctor {
        /// Print the report as JSON
        #[arg(long)]
        json: bool,
        /// Only check this ACP agent (by id, e.g. `claude`)
        #[arg(long, value_name = "ID")]
        agent: Option<String>,
    },

    /// Log an ACP agent in from this terminal (Settings opens a terminal pane running this)
    AcpLogin {
        /// The agent's id, e.g. `antigravity`
        id: String,
        /// Use this login method instead of asking
        #[arg(long, value_name = "METHOD")]
        method: Option<String>,
    },

    /// Manage skills
    Skills {
        #[command(subcommand)]
        subcommand: SkillsCommands,
    },
}

#[derive(Subcommand)]
pub(crate) enum SkillsCommands {
    /// List all skills
    List,

    /// Create a new skill
    Create {
        /// Skill name
        name: String,
        /// Skill description
        description: String,
        /// Skill content
        content: String,
    },

    /// Update an existing skill
    Update {
        /// Skill name
        name: String,
        /// New description
        description: String,
        /// New content
        content: String,
    },

    /// Delete a skill
    Delete {
        /// Skill name
        name: String,
    },

    /// Show a skill
    Show {
        /// Skill name
        name: String,
    },
}
