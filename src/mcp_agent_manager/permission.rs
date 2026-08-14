//! The permission mode a dispatched turn runs under.
//!
//! Every runner has one and none of them agree on the spelling: claude says
//! `bypassPermissions`, codex says `agent-full-access`, opencode fills the same slot with
//! its own agents. So the mode a caller may name is not a fixed enum here — it is whatever
//! the target runner publishes, which is exactly what `agent_runner_options` already
//! reports as `permission_modes`.
//!
//! Constructing one is the only way to attach a mode to a [`super::dispatch::Dispatch`],
//! and construction goes through the runner's own list. A mode the target does not
//! recognise is otherwise the worst kind of failure: stored, pushed, silently ignored, and
//! the agent runs the whole conversation asking a human who is not watching.

use anyhow::{bail, Result};

use crate::runner::{RunnerKind, RunnerRegistry};

/// A permission mode the target runner actually offers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PermissionMode(Box<str>);

impl PermissionMode {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }

    /// `requested`, checked against the modes `kind` publishes.
    ///
    /// A runner that publishes none cannot contradict the caller, so the value passes
    /// through: refusing there would make the argument unusable against any engine whose
    /// catalogue route is missing or momentarily unreachable.
    pub(super) async fn resolve(
        registry: &RunnerRegistry,
        kind: RunnerKind,
        directory: &str,
        requested: &str,
    ) -> Result<Self> {
        let requested = requested.trim();
        if requested.is_empty() {
            bail!(
                "'permission' was empty. Omit it to inherit, or pass one of the modes \
                 agent_runner_options reports under 'permission_modes'."
            );
        }
        let offered = registry.permission_modes(kind, directory).await;
        if offered.is_empty() || offered.iter().any(|mode| mode == requested) {
            return Ok(Self(requested.into()));
        }
        bail!(
            "'{requested}' is not a permission mode this runner offers. It accepts: {}. \
             Call agent_runner_options for what each one means.",
            offered.join(", ")
        )
    }

    /// The mode the calling session is already running under.
    ///
    /// Only when the new session lands on the same runner: a mode is a name in one
    /// runner's vocabulary, and `bypassPermissions` means nothing to codex. Inheriting is
    /// what stops a fleet started by an unattended agent from sitting on permission
    /// prompts that nobody is there to answer, while still never granting the child more
    /// than the parent already had.
    pub(super) async fn inherited(
        registry: &RunnerRegistry,
        kind: &RunnerKind,
        directory: &str,
        source: &str,
    ) -> Option<Self> {
        if source.is_empty() || registry.runner_for(source).await != *kind {
            return None;
        }
        let choices = registry.session_engine(source, directory).await.ok()?;
        choices
            .permission_mode
            .map(|mode| Self(mode.as_str().into()))
    }
}

#[cfg(test)]
#[path = "permission_tests.rs"]
mod permission_tests;
