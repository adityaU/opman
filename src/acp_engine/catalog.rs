//! Every harness opman knows speaks ACP, and how to start it.
//!
//! The registry in [`super::config`] used to ship two agents, so the two were written out
//! by hand. That does not scale to the protocol's published agent list, and more to the
//! point it made "opman supports this harness" a code change per harness. This table is the
//! answer: one row per agent, all of them *declared*, and only the two opman is developed
//! against *enabled*. A fresh install therefore spawns exactly the processes it did before
//! the catalogue existed — the rest are one toggle away in settings, and the toggle takes
//! effect without a restart because the supervisor reconciles against config.
//!
//! Rows whose upstream docs do not state a launch command ship as [`Launch::Undocumented`],
//! which resolves to an empty command. That is already the "nothing to launch" case
//! everywhere else in the engine ([`AgentConfig::launchable`] is false), so such a row can
//! never half-start: it lists itself, links its documentation, and waits for the command to
//! be filled in on the settings page. Shipping a guessed command instead would spawn the
//! wrong binary under the agent's name, which is worse than shipping none.

use super::config::AgentConfig;

/// The npm package implementing Claude's ACP server. `@zed-industries/claude-code-acp`
/// is the old name and is deprecated; this one is its rename.
const CLAUDE_ACP_PACKAGE: &str = "@agentclientprotocol/claude-agent-acp@latest";

/// The ACP adapter for Codex. The `codex` CLI speaks its own app-server JSON-RPC and no
/// ACP, so the adapter is a separate package rather than a subcommand. As with Claude,
/// the `@zed-industries/*` name is the deprecated one; this is its rename.
const CODEX_ACP_PACKAGE: &str = "@agentclientprotocol/codex-acp@latest";

#[path = "catalog_entries.rs"]
pub mod entries;

pub use entries::ENTRIES;

/// How an agent's ACP server is started.
#[derive(Clone, Copy)]
pub enum Launch {
    /// Published to npm, run through the node runner: `npx -y <package>`.
    Npm(&'static str),
    /// Published to PyPI, run through `uvx`.
    Uvx(&'static str),
    /// A binary the user installs themselves, invoked by name off `PATH`.
    Bin(&'static str),
    /// Implements ACP, but its published documentation does not state the command. The
    /// row ships anyway so the agent is a form-fill rather than a code change.
    Undocumented,
}

impl Launch {
    /// The executable and its leading arguments. An empty command marks the agent
    /// unlaunchable, which is exactly what [`Launch::Undocumented`] means.
    fn resolve(self) -> (String, Vec<String>) {
        match self {
            Self::Npm(package) => (node_runner(), vec!["-y".to_string(), package.to_string()]),
            Self::Uvx(package) => ("uvx".to_string(), vec![package.to_string()]),
            Self::Bin(binary) => (binary.to_string(), Vec::new()),
            Self::Undocumented => (String::new(), Vec::new()),
        }
    }
}

/// One harness, as opman ships it.
pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    launch: Launch,
    /// Arguments after the ones [`Launch`] itself contributes — usually the subcommand or
    /// flag that puts the CLI into ACP mode.
    args: &'static [&'static str],
    /// Where the launch command above was read from, so a row that needs correcting says
    /// where to look. Shown next to the agent in settings.
    pub docs: &'static str,
    /// Started without being asked. True only for the agents opman is developed against.
    enabled: bool,
}

/// A catalogue row with opman's defaults: declared, off, no behavioural tuning. The two
/// enabled agents are written out longhand below instead, because every field of theirs is
/// a decision worth reading.
const fn row(
    id: &'static str,
    name: &'static str,
    launch: Launch,
    args: &'static [&'static str],
    docs: &'static str,
) -> Entry {
    Entry {
        id,
        name,
        launch,
        args,
        docs,
        enabled: false,
    }
}

/// How to run an npm-published ACP server. Overridable for offline installs that have
/// the package vendored (`OPMAN_ACP_NPX=/path/to/claude-agent-acp`, no args needed).
fn node_runner() -> String {
    std::env::var("OPMAN_ACP_NPX").unwrap_or_else(|_| "npx".to_string())
}

/// Whether opman ships this agent, so removing its entry restores it rather than
/// deleting it.
pub fn is_builtin(id: &str) -> bool {
    ENTRIES.iter().any(|entry| entry.id == id)
}

/// Where this agent's launch command was documented, if opman ships the agent.
pub fn docs_for(id: &str) -> Option<&'static str> {
    ENTRIES
        .iter()
        .find(|entry| entry.id == id)
        .map(|entry| entry.docs)
}

impl Entry {
    /// This row as the engine reads it.
    pub fn config(&self) -> AgentConfig {
        let (command, mut args) = self.launch.resolve();
        args.extend(self.args.iter().map(|arg| (*arg).to_string()));
        let base = AgentConfig {
            display_name: self.name.to_string(),
            command,
            args,
            runner: self.id.to_string(),
            enabled: self.enabled,
            ..AgentConfig::default()
        };
        tuned(self.id, base)
    }
}

/// The per-agent facts that are not "how do I start it".
///
/// Only the two enabled agents have any, and both are things opman learned by driving
/// them rather than anything the protocol reports — which is why they live next to the
/// catalogue instead of being asked for over ACP.
fn tuned(id: &str, mut config: AgentConfig) -> AgentConfig {
    match id {
        "claude" => {
            config.enabled = true;
            // Claude's adapter is a full agent: it owns its file and terminal tools, and
            // opman renders those tool calls directly from `tool_call` updates.
            config.default_mode = "bypassPermissions".to_string();
            // Claude's ACP sessionId is the UUID of the transcript it writes under
            // `~/.claude/projects`, which is where its subagent conversations live.
            config.subagent_transcripts = true;
        }
        // No `default_mode`: Codex's ACP modes are approval policies
        // (`read-only`/`agent`/`agent-full-access`), not agents, and the adapter already
        // opens on `agent` — read, edit and run inside the workspace without prompting.
        // Naming it here would only restate the agent's own default.
        "codex" => config.enabled = true,
        // opencode fills the ACP `mode` slot with its own agents (`build`/`plan`) rather
        // than with permission modes, so those belong in the agent picker.
        "opencode" => {
            config.modes_are_agents = true;
            config.default_mode = "build".to_string();
        }
        _ => {}
    }
    config
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod catalog_tests;
