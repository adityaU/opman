//! Declarative agent registry for the ACP engine.
//!
//! Adding an ACP server to opman is a config edit, never a code change: every
//! agent-specific fact (how to launch it, what it may ask opman to do, which runner
//! slot it occupies) lives in one JSON file. The engine itself only speaks ACP.
//!
//! Resolution order, later winning per-agent:
//! 1. the built-in defaults below (which ship the `claude` agent), then
//! 2. `$OPMAN_ACP_CONFIG`, else `~/.config/opman/acp.json`.
//!
//! An entry present in both is merged field-by-field, so a user file that only sets
//! `{"agents":{"claude":{"enabled":false}}}` keeps the default launch command.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

/// The npm package implementing Claude's ACP server. `@zed-industries/claude-code-acp`
/// is the old name and is deprecated; this one is its rename.
const CLAUDE_ACP_PACKAGE: &str = "@agentclientprotocol/claude-agent-acp@latest";

/// Environment variables stripped from every ACP child by default. Claude's adapter
/// aborts `session/new` with "cannot be launched inside another Claude Code session"
/// when it inherits `CLAUDECODE` from an opman that was itself started from a session.
const DEFAULT_ENV_REMOVE: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_CODE_ENTRYPOINT",
];

/// What the client (opman) tells the agent it is willing to do on the agent's behalf.
/// Off by default: an agent that cannot delegate file and terminal work uses its own
/// built-in tools, which is the behaviour opman already renders.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientCaps {
    pub read_text_file: bool,
    pub write_text_file: bool,
    pub terminal: bool,
}

/// One ACP server opman can drive.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentConfig {
    /// Label for the engine picker. Defaults to the agent id.
    pub display_name: String,
    /// Executable to spawn. Empty disables the agent (nothing to launch).
    pub command: String,
    pub args: Vec<String>,
    /// Extra environment for the child.
    pub env: BTreeMap<String, String>,
    /// Inherited variables to strip from the child, on top of [`DEFAULT_ENV_REMOVE`].
    pub env_remove: Vec<String>,
    /// Runner slot this agent occupies. Empty means "the agent id".
    pub runner: String,
    pub client_caps: ClientCaps,
    /// Forward opman's own MCP servers (terminal, neovim, time, ui, agent-manager)
    /// to the agent via `session/new`.
    pub inject_mcp: bool,
    /// Initial `mode` config option / session mode, e.g. `bypassPermissions`.
    pub default_mode: String,
    /// Initial `model` config option.
    pub default_model: String,
    /// What the agent's ACP `mode` slot actually means. ACP calls it a mode and leaves the
    /// meaning to the agent: Claude fills it with permission modes
    /// (`default`/`acceptEdits`/`plan`/…), while opencode fills it with its *agents*
    /// (`build`/`plan`). Set this when the latter is true, and opman lists those values in
    /// the agent picker instead of the permission dropdown — otherwise choosing an "agent"
    /// would claim to change permissions it has no say over.
    pub modes_are_agents: bool,
    /// Treat the ACP `sessionId` as a Claude transcript UUID, so `Task` tool calls can
    /// be nested as child sessions read from `~/.claude/projects`. ACP has no concept of
    /// subagent sessions, so this is the one agent-specific enrichment opman keeps — off
    /// unless the agent is known to write Claude-format transcripts.
    pub subagent_transcripts: bool,
    pub enabled: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            env_remove: Vec::new(),
            runner: String::new(),
            client_caps: ClientCaps::default(),
            inject_mcp: true,
            default_mode: String::new(),
            default_model: String::new(),
            modes_are_agents: false,
            subagent_transcripts: false,
            enabled: true,
        }
    }
}

impl AgentConfig {
    /// Every variable to unset on the child: the built-in list plus config additions.
    pub fn env_removals(&self) -> impl Iterator<Item = &str> {
        DEFAULT_ENV_REMOVE
            .iter()
            .copied()
            .chain(self.env_remove.iter().map(String::as_str))
    }

    /// True when this agent is usable: enabled and has something to launch.
    pub fn launchable(&self) -> bool {
        self.enabled && !self.command.is_empty()
    }

    /// Overlay `other`'s explicitly-set fields onto self. Absent JSON fields arrive as
    /// their `Default`, so "explicitly set" is approximated as "differs from default" —
    /// which is what lets a partial user entry keep the built-in launch command.
    fn overlay(&mut self, other: AgentConfig) {
        let d = AgentConfig::default();
        if !other.display_name.is_empty() {
            self.display_name = other.display_name;
        }
        if !other.command.is_empty() {
            self.command = other.command;
        }
        if !other.args.is_empty() {
            self.args = other.args;
        }
        if !other.env.is_empty() {
            self.env.extend(other.env);
        }
        if !other.env_remove.is_empty() {
            self.env_remove = other.env_remove;
        }
        if !other.runner.is_empty() {
            self.runner = other.runner;
        }
        if other.client_caps != d.client_caps {
            self.client_caps = other.client_caps;
        }
        if other.inject_mcp != d.inject_mcp {
            self.inject_mcp = other.inject_mcp;
        }
        if !other.default_mode.is_empty() {
            self.default_mode = other.default_mode;
        }
        if !other.default_model.is_empty() {
            self.default_model = other.default_model;
        }
        if other.modes_are_agents != d.modes_are_agents {
            self.modes_are_agents = other.modes_are_agents;
        }
        if other.subagent_transcripts != d.subagent_transcripts {
            self.subagent_transcripts = other.subagent_transcripts;
        }
        if other.enabled != d.enabled {
            self.enabled = other.enabled;
        }
    }
}

/// The whole `acp.json` document.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AcpConfig {
    pub agents: BTreeMap<String, AgentConfig>,
}

/// Built-in defaults. Claude ships configured so a fresh install needs no config file;
/// the file exists to override it or to add other ACP servers.
fn builtin() -> AcpConfig {
    let claude = AgentConfig {
        display_name: "Claude".to_string(),
        command: node_runner(),
        args: vec!["-y".to_string(), CLAUDE_ACP_PACKAGE.to_string()],
        runner: "claude".to_string(),
        // Claude's adapter is a full agent: it owns its file and terminal tools, and
        // opman renders those tool calls directly from `tool_call` updates.
        client_caps: ClientCaps::default(),
        inject_mcp: true,
        default_mode: "bypassPermissions".to_string(),
        // Claude's ACP sessionId is the UUID of the transcript it writes under
        // `~/.claude/projects`, which is where its subagent conversations live.
        subagent_transcripts: true,
        ..AgentConfig::default()
    };
    AcpConfig {
        agents: BTreeMap::from([("claude".to_string(), claude)]),
    }
}

/// How to run an npm-published ACP server. Overridable for offline installs that have
/// the package vendored (`OPMAN_ACP_NPX=/path/to/claude-agent-acp`, no args needed).
fn node_runner() -> String {
    std::env::var("OPMAN_ACP_NPX").unwrap_or_else(|_| "npx".to_string())
}

/// Path of the user config file, if one is configured or the default location exists.
pub fn config_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("OPMAN_ACP_CONFIG") {
        if !explicit.is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }
    dirs::config_dir().map(|d| d.join("opman").join("acp.json"))
}

/// Built-in defaults with the user's file merged over them. A malformed or missing file
/// is not fatal: opman still starts with the built-in agents.
pub fn load() -> AcpConfig {
    let mut cfg = builtin();
    let Some(path) = config_path() else {
        return finish(cfg);
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return finish(cfg);
    };
    match serde_json::from_str::<AcpConfig>(&raw) {
        Ok(user) => {
            for (id, entry) in user.agents {
                cfg.agents.entry(id).or_default().overlay(entry);
            }
        }
        Err(e) => tracing::warn!(path = %path.display(), "ignoring malformed ACP config: {e}"),
    }
    finish(cfg)
}

/// Fill in the defaults that are derived from the agent id rather than written out.
fn finish(mut cfg: AcpConfig) -> AcpConfig {
    for (id, entry) in cfg.agents.iter_mut() {
        if entry.display_name.is_empty() {
            entry.display_name = id.clone();
        }
        if entry.runner.is_empty() {
            entry.runner = id.clone();
        }
    }
    cfg
}

impl AcpConfig {
    /// Launchable agents, in stable id order.
    pub fn active(&self) -> impl Iterator<Item = (&String, &AgentConfig)> {
        self.agents.iter().filter(|(_, a)| a.launchable())
    }

    /// The launchable agent occupying a runner slot, if any.
    pub fn for_runner(&self, runner: &str) -> Option<(&String, &AgentConfig)> {
        self.active().find(|(_, a)| a.runner == runner)
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod config_tests;
