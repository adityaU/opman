//! What a dispatched turn runs as.
//!
//! `model` and `effort` are required, and this type is why: every path that starts a turn
//! takes a [`Dispatch`], so there is no way to reach [`crate::runner::RunnerRegistry`]
//! having forgotten one. The old shape — two `Option<String>`s threaded through send and
//! start — made "no model at all" the easiest thing to write, and an omitted model does
//! not mean the default. It means the target session's last model, which the calling agent
//! cannot see and did not choose.

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::app::EngineChoices;

use super::permission::PermissionMode;

/// The model, reasoning effort, and provider one turn runs under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dispatch {
    model: Box<str>,
    effort: Box<str>,
    /// Optional: OpenCode wants a provider id alongside the model, and the other runners
    /// derive it. An empty one is the same as none.
    provider: Option<Box<str>>,
    /// Optional, and genuinely so: `None` leaves the target on the mode it is already set
    /// to, which is the right answer for steering a session someone else configured.
    permission: Option<PermissionMode>,
}

impl Dispatch {
    /// Build one, refusing the halves that cannot be guessed.
    ///
    /// The error names `agent_runner_options` because that is the tool that answers it —
    /// an agent that reads "model is required" and has no idea what models exist has been
    /// told the rule and not the remedy.
    pub(crate) fn parse(
        model: Option<&str>,
        effort: Option<&str>,
        provider: Option<&str>,
    ) -> Result<Self> {
        let model = present(model).context(
            "'model' is required. Call agent_runner_options to list the models this runner \
             accepts, then pass one.",
        )?;
        let effort = present(effort).context(
            "'effort' is required. Call agent_runner_options to list the reasoning efforts \
             the chosen model supports, then pass one.",
        )?;
        Ok(Self {
            model: model.into(),
            effort: effort.into(),
            provider: present(provider).map(Into::into),
            permission: None,
        })
    }

    /// The same dispatch, run under `permission`.
    pub(crate) fn with_permission(mut self, permission: Option<PermissionMode>) -> Self {
        self.permission = permission;
        self
    }

    /// This dispatch and `message` as one runner-neutral request body.
    ///
    /// `effort` sits at the top level because that is the shape every runner already
    /// reads: OpenCode renames it to `variant`, Codex forwards it to `turn/start`, and the
    /// Claude engine turns it into `--effort`. `permission` is only present when one was
    /// chosen — an absent field leaves the session's own mode alone, where an empty one
    /// would read as a choice of nothing.
    pub(crate) fn body(&self, message: &str) -> Value {
        let mut body = json!({
            "parts": [{ "type": "text", "text": message }],
            "model": {
                "providerID": self.provider.as_deref().unwrap_or_default(),
                "modelID": self.model.as_ref(),
            },
            "effort": self.effort.as_ref(),
        });
        let (Some(permission), Some(object)) = (self.permission.as_ref(), body.as_object_mut())
        else {
            return body;
        };
        object.insert(
            "permission".to_string(),
            Value::String(permission.as_str().to_string()),
        );
        body
    }

    /// The permission mode this turn runs under, if one was settled at all.
    pub(crate) fn permission(&self) -> Option<&str> {
        self.permission.as_ref().map(PermissionMode::as_str)
    }

    /// This dispatch as the configuration to record on a session.
    ///
    /// The send path applies the same values, but only when a turn follows. A session
    /// started without an opening message has no turn to carry them, and recording them
    /// here is what makes its *first* connection open in the chosen mode rather than in
    /// whatever the agent defaults to.
    pub(crate) fn choices(&self) -> EngineChoices {
        EngineChoices::from_parts(
            Some(&self.model),
            None,
            Some(&self.effort),
            self.permission(),
        )
    }
}

/// A value that is there and is not just spaces.
fn present(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod dispatch_tests;
