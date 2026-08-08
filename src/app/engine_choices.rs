//! What a session was last configured to run as: model, agent, effort, permission mode.
//!
//! These are not opman's to store. Every runner already keeps them per session and writes
//! them to its own disk — the ACP engine in [`crate::acp_engine::Session`], the claude
//! engine in its `SessionEntry` — because each one has to reproduce the same configuration
//! when it resumes a conversation. This is only the shape they travel in, from the engine
//! that owns them, through the session list, to the composer that displays them.
//!
//! Keeping opman out of the storage business is the point. A second copy would be a second
//! answer to "what is this session set to", and the two would disagree the first time a
//! session was configured anywhere other than the composer.

use serde::{Deserialize, Deserializer, Serialize};

/// One value the user picked, guaranteed to be something.
///
/// The empty string is how the runners spell "not set", and it arrives that way from a
/// hand-edited config, an older persisted record, or an engine that fills a field it has
/// no answer for. Reading one as a choice puts a selection in the composer that matches
/// nothing in the catalogue — so it is rejected at construction and cannot be built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Choice(Box<str>);

impl Choice {
    /// The choice in `value`, or `None` when it holds nothing.
    pub fn new(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| Self(trimmed.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Choice {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Read a chosen value, treating an empty or absent one as "never chosen".
///
/// A malformed entry must not fail the whole session list: one bad field in one session
/// would empty the sidebar for every project. It reads as unset instead, which is the
/// state the session is genuinely in.
fn optional_choice<'de, D>(deserializer: D) -> Result<Option<Choice>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.as_deref().and_then(Choice::new))
}

/// The engine configuration of one session, as its runner reports it.
///
/// Every field is optional because "never chosen" is a real, distinct state: a runner
/// answers `None` with its own current default, which is not the same as being pinned to
/// whatever that default happened to be on the day the session was created.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineChoices {
    #[serde(default, deserialize_with = "optional_choice")]
    pub model: Option<Choice>,
    #[serde(default, deserialize_with = "optional_choice")]
    pub agent: Option<Choice>,
    #[serde(default, deserialize_with = "optional_choice")]
    pub effort: Option<Choice>,
    #[serde(
        default,
        rename = "permissionMode",
        deserialize_with = "optional_choice"
    )]
    pub permission_mode: Option<Choice>,
}

impl EngineChoices {
    /// Build from whatever an engine holds, dropping the fields it has no answer for.
    pub fn from_parts(
        model: Option<&str>,
        agent: Option<&str>,
        effort: Option<&str>,
        permission_mode: Option<&str>,
    ) -> Self {
        Self {
            model: model.and_then(Choice::new),
            agent: agent.and_then(Choice::new),
            effort: effort.and_then(Choice::new),
            permission_mode: permission_mode.and_then(Choice::new),
        }
    }

    /// Whether the session has never been configured at all.
    pub fn is_empty(&self) -> bool {
        self.model.is_none()
            && self.agent.is_none()
            && self.effort.is_none()
            && self.permission_mode.is_none()
    }
}

#[cfg(test)]
#[path = "engine_choices_tests.rs"]
mod engine_choices_tests;
