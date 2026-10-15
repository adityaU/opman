//! The runners opman serves with its own engines rather than through ACP.

use crate::cli::AgentBackend;
use crate::runner::RunnerKind;

/// A runner opman serves itself: `opencode` (an `opencode serve` child) and `claude-code`
/// (the in-process adapter over Claude Code's background agents).
///
/// ACP agents are switched on and off in `acp.json`, next to the rest of their definition.
/// These two have no definition to edit — only whether they are on — so they get a file of
/// their own; see [`super::config`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BuiltinRunner {
    Opencode,
    ClaudeCode,
}

impl BuiltinRunner {
    pub const ALL: [Self; 2] = [Self::Opencode, Self::ClaudeCode];

    /// The runner label, as sessions and the web UI spell it.
    pub fn id(self) -> &'static str {
        match self {
            Self::Opencode => "opencode",
            Self::ClaudeCode => "claude-code",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Opencode => "OpenCode",
            Self::ClaudeCode => "Claude Code",
        }
    }

    pub fn kind(self) -> RunnerKind {
        match self {
            Self::Opencode => RunnerKind::Opencode,
            Self::ClaudeCode => RunnerKind::ClaudeCode,
        }
    }

    pub fn from_kind(kind: &RunnerKind) -> Option<Self> {
        match kind {
            RunnerKind::Opencode => Some(Self::Opencode),
            RunnerKind::ClaudeCode => Some(Self::ClaudeCode),
            RunnerKind::Claude | RunnerKind::Acp(_) => None,
        }
    }

    /// Accepts the same spellings as [`RunnerKind::parse`].
    pub fn parse(raw: &str) -> Option<Self> {
        RunnerKind::parse(raw).as_ref().and_then(Self::from_kind)
    }

    /// The CLI backend that names this runner, for code that still speaks in backends.
    pub fn backend(self) -> AgentBackend {
        match self {
            Self::Opencode => AgentBackend::Opencode,
            Self::ClaudeCode => AgentBackend::ClaudeCode,
        }
    }

    /// The binary whose `--version` says whether this runner can start at all.
    pub fn binary(self) -> String {
        match self {
            Self::Opencode => "opencode".to_string(),
            Self::ClaudeCode => {
                std::env::var("OPMAN_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string())
            }
        }
    }
}
