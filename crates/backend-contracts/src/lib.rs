#![deny(unsafe_code)]
#![deny(clippy::unwrap_used)]

use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A runner that can own a session. This is intentionally not `Copy`: runner
/// selection is a value with an explicit ownership boundary, not a freely
/// duplicated flag.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunnerKind {
    #[default]
    Opencode,
    #[serde(rename = "claude-code")]
    ClaudeCode,
    Claude,
    Codex,
}

impl RunnerKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "opencode" | "open-code" => Some(Self::Opencode),
            "claude-code" | "claudecode" => Some(Self::ClaudeCode),
            "claude" | "claude-p" | "claudep" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Opencode => "opencode",
            Self::ClaudeCode => "claude-code",
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
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

    #[test]
    fn runner_names_are_explicit() {
        assert_eq!(RunnerKind::parse("claude-code"), Some(RunnerKind::Claude));
        assert_eq!(RunnerKind::Codex.display_name(), "codex");
    }

    #[test]
    fn invalid_identity_values_are_rejected() {
        assert!(SessionId::new(" ").is_err());
        assert!(ProjectDirectory::new("relative/path").is_err());
    }
}
