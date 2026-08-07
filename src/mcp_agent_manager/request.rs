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
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) message: Option<String>,
    #[serde(default)]
    pub(super) delivery: Option<String>,
}

/// What the caller is asking for. One variant per tool, so the match in `mod.rs` is
/// exhaustive rather than a string comparison with a fallthrough.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Op {
    Send,
    Start,
    Progress,
    Options,
}

impl ManagerRequest {
    pub(super) fn op(&self) -> Result<Op> {
        match self.op.as_str() {
            "send" => Ok(Op::Send),
            "start" => Ok(Op::Start),
            "progress" => Ok(Op::Progress),
            "options" => Ok(Op::Options),
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
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod request_tests;
