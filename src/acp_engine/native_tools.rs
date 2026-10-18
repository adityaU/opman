//! Which of an agent's own built-in tools are switched off.
//!
//! An ACP agent ships its own tools (a shell, file editing, web access). Some people want
//! those off so the agent reaches for opman's MCP tools instead. How a harness is told to
//! drop a tool differs per harness, so this module holds only the shared vocabulary:
//! the user's choice ([`NativeTools`]), and what the catalogue knows about a harness
//! ([`ToolSet`]). Turning a choice into launch environment or session metadata is in
//! [`super::native_tools_render`]; the per-harness data is in
//! [`super::native_tools_catalog`].

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// The user's choice for one agent. Stored in `acp.json` as `"all"`, `"off"` or
/// `{"disabled": ["Bash", ...]}`; a file without the key means [`NativeTools::All`].
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeTools {
    /// Every native tool stays on. The agent runs exactly as it would outside opman.
    #[default]
    All,
    /// Every native tool the catalogue can switch off is off.
    Off,
    /// Only these tool ids are off. Ids the harness does not list are ignored.
    Disabled(BTreeSet<String>),
}

impl NativeTools {
    /// The tools of `set` this choice switches off, in catalogue order.
    pub fn disabled(&self, set: &ToolSet) -> Vec<&'static ToolDef> {
        set.tools
            .iter()
            .filter(|tool| self.turns_off(tool.id))
            .collect()
    }

    /// Whether the tool with this id is switched off.
    pub fn turns_off(&self, id: &str) -> bool {
        match self {
            Self::All => false,
            Self::Off => true,
            Self::Disabled(ids) => ids.contains(id),
        }
    }

    /// The same choice in its one canonical spelling for `set`: unknown ids dropped, an
    /// empty list is `All`, and a list naming every tool is `Off`. Two spellings of one
    /// state would make the page show a mixed switch for what is really "all off".
    pub fn canonical(self, set: &ToolSet) -> Self {
        let Self::Disabled(ids) = self else {
            return self;
        };
        let known: BTreeSet<String> = set
            .tools
            .iter()
            .filter(|tool| ids.contains(tool.id))
            .map(|tool| tool.id.to_string())
            .collect();
        match known.len() {
            0 => Self::All,
            len if len == set.tools.len() => Self::Off,
            _ => Self::Disabled(known),
        }
    }
}

/// One built-in tool of a harness.
#[derive(Debug, Eq, PartialEq)]
pub struct ToolDef {
    /// The harness's own name for the tool. It is what goes into the switch, so it is
    /// also what `acp.json` stores.
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

/// How opman tells a harness to drop tools. Each variant is a mechanism that was run
/// against the real agent, not a guess.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mechanism {
    /// Claude: `_meta.claudeCode.options.disallowedTools` on `session/new` and
    /// `session/load`. A disallowed tool is not offered to the model at all.
    ClaudeSessionMeta,
    /// Antigravity: `_meta.antigravity.disabledTools` on `session/new` and `session/load`.
    /// A disabled tool is not offered to the model, which then says it has no such tool.
    AntigravityMeta,
    /// opencode: `permission.<tool> = "deny"` merged into `OPENCODE_CONFIG_CONTENT`.
    OpencodePermission,
    /// Codex: a JSON object in `CODEX_CONFIG`, merged into the session config.
    CodexConfig,
}

impl Mechanism {
    /// One line for the settings page, saying where the switch is applied.
    pub fn note(self) -> &'static str {
        match self {
            Self::ClaudeSessionMeta => "Sent to Claude as disallowed tools when a session starts.",
            Self::AntigravityMeta => "Sent to Antigravity as disabled tools when a session starts.",
            Self::OpencodePermission => "Set as deny rules in the opencode config for the agent.",
            Self::CodexConfig => "Set in the Codex config for the agent.",
        }
    }
}

/// What the catalogue knows about one harness's tools.
#[derive(Debug, Eq, PartialEq)]
pub struct ToolSet {
    pub mechanism: Mechanism,
    pub tools: &'static [ToolDef],
}

#[cfg(test)]
#[path = "native_tools_tests.rs"]
mod native_tools_tests;
