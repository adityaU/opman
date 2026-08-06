#![deny(unsafe_code)]
#![deny(clippy::unwrap_used)]

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

/// ACP agent ids registered from `acp.json` at startup.
///
/// The set of runners is genuinely open now: an ACP agent is declared in config, so opman
/// cannot know the names at compile time. It must still reject a name nobody declared —
/// `parse` is what validates runner labels arriving from users and from the web UI — so the
/// dynamic names are registered here rather than accepted on faith.
static ACP_RUNNERS: RwLock<BTreeSet<String>> = RwLock::new(BTreeSet::new());

/// Declare the ACP agent ids that exist. Called once per process, after config load.
pub fn register_acp_runners(ids: impl IntoIterator<Item = String>) {
    let Ok(mut registered) = ACP_RUNNERS.write() else {
        return;
    };
    registered.extend(ids.into_iter().filter(|id| is_valid_acp_id(id)));
}

fn acp_registered(id: &str) -> bool {
    ACP_RUNNERS
        .read()
        .map(|set| set.contains(id))
        .unwrap_or(false)
}

/// Config ids are used as runner labels, provider ids and file-name fragments, so keep them
/// to a conservative shape rather than trusting whatever the config file holds.
fn is_valid_acp_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

/// A runner that can own a session. This is intentionally not `Copy`: runner
/// selection is a value with an explicit ownership boundary, not a freely
/// duplicated flag.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub enum RunnerKind {
    #[default]
    Opencode,
    ClaudeCode,
    Claude,
    Codex,
    /// An ACP agent declared in `acp.json`, identified by its config id.
    Acp(String),
}

impl RunnerKind {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        match value.as_str() {
            "opencode" | "open-code" => Some(Self::Opencode),
            "claude-code" | "claudecode" => Some(Self::ClaudeCode),
            // `claude` keeps its own variant: it is the slot the built-in ACP agent
            // occupies, and persisted session bindings and UI labels already use it.
            "claude" | "claude-p" | "claudep" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            other if acp_registered(other) => Some(Self::Acp(other.to_string())),
            _ => None,
        }
    }

    pub fn display_name(&self) -> Cow<'_, str> {
        match self {
            Self::Opencode => Cow::Borrowed("opencode"),
            Self::ClaudeCode => Cow::Borrowed("claude-code"),
            Self::Claude => Cow::Borrowed("claude"),
            Self::Codex => Cow::Borrowed("codex"),
            Self::Acp(id) => Cow::Borrowed(id.as_str()),
        }
    }
}

impl Display for RunnerKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.display_name())
    }
}

impl Serialize for RunnerKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.display_name())
    }
}

impl<'de> Deserialize<'de> for RunnerKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if let Some(kind) = Self::parse(&raw) {
            return Ok(kind);
        }
        // Persisted state, unlike user input, may name an agent whose config entry has since
        // been removed. Preserve it rather than failing the whole load; the runner simply
        // will not resolve to an engine.
        let id = raw.trim().to_ascii_lowercase();
        if is_valid_acp_id(&id) {
            return Ok(Self::Acp(id));
        }
        Err(serde::de::Error::custom(format!("unknown runner '{raw}'")))
    }
}

/// A validated runner-native session identifier.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionId(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidSessionId;

impl Display for InvalidSessionId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("session id must not be empty or whitespace")
    }
}

impl std::error::Error for InvalidSessionId {}

impl TryFrom<String> for SessionId {
    type Error = InvalidSessionId;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.trim().is_empty() {
            return Err(InvalidSessionId);
        }
        Ok(Self(value))
    }
}

impl SessionId {
    pub fn new(value: &str) -> Result<Self, InvalidSessionId> {
        value.to_owned().try_into()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Display for SessionId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for SessionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

/// A validated project directory. Runner processes must never receive an
/// empty path or an unresolved relative path.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectDirectory(PathBuf);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidProjectDirectory;

impl Display for InvalidProjectDirectory {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("project directory must be an absolute path")
    }
}

impl std::error::Error for InvalidProjectDirectory {}

impl TryFrom<PathBuf> for ProjectDirectory {
    type Error = InvalidProjectDirectory;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        if value.as_os_str().is_empty() || !value.is_absolute() {
            return Err(InvalidProjectDirectory);
        }
        Ok(Self(value))
    }
}

impl ProjectDirectory {
    pub fn new(value: impl AsRef<Path>) -> Result<Self, InvalidProjectDirectory> {
        value.as_ref().to_path_buf().try_into()
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn as_str(&self) -> Option<&str> {
        self.0.to_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `claude` and `claude-code` are different runners with different engines.
    /// Collapsing them routes a session's turns to the wrong process — or, worse,
    /// reads as a runner switch and forks the session.
    #[test]
    fn runner_names_are_explicit() {
        assert_eq!(RunnerKind::parse("claude"), Some(RunnerKind::Claude));
        assert_eq!(
            RunnerKind::parse("claude-code"),
            Some(RunnerKind::ClaudeCode)
        );
        assert_eq!(
            RunnerKind::parse("CLAUDE-CODE "),
            Some(RunnerKind::ClaudeCode)
        );
        assert_eq!(RunnerKind::parse("nope"), None);
        assert_eq!(RunnerKind::Codex.display_name(), "codex");
        // Every name must round-trip through the label the web UI sends back.
        for kind in [
            RunnerKind::Opencode,
            RunnerKind::ClaudeCode,
            RunnerKind::Claude,
            RunnerKind::Codex,
        ] {
            assert_eq!(RunnerKind::parse(&kind.display_name()), Some(kind.clone()));
            assert_eq!(
                serde_json::from_str::<RunnerKind>(&format!("\"{}\"", kind.display_name())).ok(),
                Some(kind),
            );
        }
    }

    /// An ACP agent is only a runner once its config declared it. Accepting any string
    /// would turn a typo in a runner label into a session bound to an engine that does
    /// not exist.
    #[test]
    fn acp_runners_must_be_registered_to_parse() {
        assert_eq!(RunnerKind::parse("gemini-acp"), None);
        register_acp_runners(["gemini-acp".to_string()]);
        assert_eq!(
            RunnerKind::parse("gemini-acp"),
            Some(RunnerKind::Acp("gemini-acp".to_string()))
        );
        let kind = RunnerKind::Acp("gemini-acp".to_string());
        assert_eq!(kind.display_name(), "gemini-acp");
        assert_eq!(
            serde_json::to_string(&kind).ok(),
            Some("\"gemini-acp\"".to_string())
        );
    }

    /// Ids are used as provider ids and file-name fragments; reject shapes that would be
    /// unsafe there even if a config file contains them.
    #[test]
    fn malformed_acp_ids_are_refused() {
        register_acp_runners(["../escape".to_string(), "Upper".to_string(), String::new()]);
        assert_eq!(RunnerKind::parse("../escape"), None);
        assert_eq!(RunnerKind::parse("Upper"), None);
        assert!(serde_json::from_str::<RunnerKind>("\"../escape\"").is_err());
    }

    /// A session persisted against an agent whose config was removed must still load.
    #[test]
    fn unregistered_but_valid_ids_survive_deserialization() {
        assert_eq!(
            serde_json::from_str::<RunnerKind>("\"retired-agent\"").ok(),
            Some(RunnerKind::Acp("retired-agent".to_string()))
        );
    }

    #[test]
    fn invalid_identity_values_are_rejected() {
        assert!(SessionId::new(" ").is_err());
        assert!(ProjectDirectory::new("relative/path").is_err());
    }
}
