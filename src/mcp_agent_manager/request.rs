//! The socket wire format, and the typed reading of it.
//!
//! The struct stays permissive — it is a JSON line from a child process, and a field that
//! is absent for one operation is required for another. Narrowing happens here instead, in
//! the accessors: an unknown `op` becomes an [`Op`] or an error, and a turn's model becomes
//! a [`Dispatch`] or an error, before either reaches the registry.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::dispatch::Dispatch;
use super::queue::Delivery;
use crate::runner::RunnerKind;

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct ManagerRequest {
    pub(super) op: String,
    #[serde(default)]
    pub(super) source_session: Option<String>,
    #[serde(default)]
    pub(super) target: Option<String>,
    #[serde(default)]
    pub(super) directory: Option<String>,
    #[serde(default)]
    pub(super) runner: Option<String>,
    #[serde(default)]
    pub(super) model: Option<String>,
    #[serde(default)]
    pub(super) effort: Option<String>,
    #[serde(default)]
    pub(super) provider: Option<String>,
    /// The permission mode the turn runs under, in the target runner's own vocabulary.
    #[serde(default)]
    pub(super) permission: Option<String>,
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) message: Option<String>,
    #[serde(default)]
    pub(super) delivery: Option<String>,
    /// Narrows `agent_runner_options` to models whose id or provider contains this.
    #[serde(default)]
    pub(super) filter: Option<String>,
    /// Seconds `agent_wait` will watch before giving up.
    #[serde(default)]
    pub(super) timeout: Option<u64>,
    /// The MCP server named by an `mcp_auth_required` notice.
    #[serde(default)]
    pub(super) server: Option<String>,
}

/// What the caller is asking for. One variant per tool, so the match in `mod.rs` is
/// exhaustive rather than a string comparison with a fallthrough.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Op {
    Send,
    Start,
    Progress,
    Options,
    Abort,
    List,
    Wait,
    /// Not a tool. The MCP proxy writes this to the same socket when a server it fronts
    /// needs the user to log in ([`crate::mcp_proxy`]), and it went unhandled long enough
    /// that every such notice came back "unknown agent manager operation".
    AuthRequired,
}

impl ManagerRequest {
    pub(super) fn op(&self) -> Result<Op> {
        match self.op.as_str() {
            "send" => Ok(Op::Send),
            "start" => Ok(Op::Start),
            "progress" => Ok(Op::Progress),
            "options" => Ok(Op::Options),
            "abort" => Ok(Op::Abort),
            "list" => Ok(Op::List),
            "wait" => Ok(Op::Wait),
            "mcp_auth_required" => Ok(Op::AuthRequired),
            other => anyhow::bail!("unknown agent manager operation: {other}"),
        }
    }

    pub(super) fn runner_kind(&self) -> Result<Option<RunnerKind>> {
        self.runner
            .as_deref()
            .map(|value| {
                RunnerKind::parse(value).with_context(|| format!("unknown runner '{value}'"))
            })
            .transpose()
    }

    pub(super) fn delivery_mode(&self) -> Result<Option<Delivery>> {
        Delivery::parse(self.delivery.as_deref())
    }

    /// The model this turn runs under. Fails when either required half is missing.
    pub(super) fn dispatch(&self) -> Result<Dispatch> {
        Dispatch::parse(
            self.model.as_deref(),
            self.effort.as_deref(),
            self.provider.as_deref(),
        )
    }

    /// The permission mode the caller named, if it named one at all.
    pub(super) fn requested_permission(&self) -> Option<&str> {
        self.permission
            .as_deref()
            .map(str::trim)
            .filter(|mode| !mode.is_empty())
    }
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod request_tests;
