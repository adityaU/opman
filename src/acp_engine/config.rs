//! Declarative agent registry for the ACP engine.
//!
//! Adding an ACP server to opman is a config edit, never a code change: every
//! agent-specific fact (how to launch it, what it may ask opman to do, which runner
//! slot it occupies) lives in one JSON file. The engine itself only speaks ACP.
//!
//! Resolution order, later winning per-agent:
//! 1. the built-in defaults below (which ship the `claude` and `codex` agents), then
//! 2. the user document in [`super::patch`].
//!
//! An entry present in both is merged field-by-field, so a user file that only sets
//! `{"agents":{"claude":{"enabled":false}}}` keeps the default launch command. This module
//! owns the *resolved* shape — every field settled, nothing optional — which is all the
//! engine ever reads.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::catalog;
use super::patch::{load_document, AcpDocument};

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
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientCaps {
    pub read_text_file: bool,
    pub write_text_file: bool,
    pub terminal: bool,
}

/// One ACP server opman can drive, with every field settled.
///
/// The user's file holds [`super::patch::AgentPatch`] instead, where a field may be absent.
/// `PartialEq` is what lets the supervisor tell an edit that needs the agent restarted from
/// one that changed nothing.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
}

/// Every agent, built-ins included, with the user's overrides already applied.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AcpConfig {
    pub agents: BTreeMap<String, AgentConfig>,
}

/// Built-in defaults: [`catalog::ENTRIES`], resolved. Every harness opman knows is
/// declared here and only the two it is developed against are enabled, so a fresh install
/// needs no config file and still starts nothing it did not start before.
fn builtin() -> AcpConfig {
    AcpConfig {
        agents: catalog::ENTRIES
            .iter()
            .map(|entry| (entry.id.to_string(), entry.config()))
            .collect(),
    }
}

/// Whether opman ships this agent, so removing its entry restores it rather than
/// deleting it.
pub fn is_builtin(id: &str) -> bool {
    catalog::is_builtin(id)
}

/// Built-in defaults with the user's file merged over them. A malformed or missing file
/// is not fatal: opman still starts with the built-in agents.
pub fn load() -> AcpConfig {
    resolve(load_document())
}

/// Built-in defaults with `document` applied. Split from [`load`] so a caller holding an
/// edited document can see what it resolves to without writing it first.
pub fn resolve(document: AcpDocument) -> AcpConfig {
    let mut cfg = builtin();
    for (id, patch) in document.agents {
        patch.apply(cfg.agents.entry(id).or_default());
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
