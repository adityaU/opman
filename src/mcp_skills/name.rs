//! A validated skill identifier.
//!
//! Every path under the skills directory is built from one of these, so directory
//! traversal is impossible by construction rather than by a check each caller has to
//! remember. That matters: the CLI's delete path reached `remove_dir_all` on whatever
//! the user typed, and the web handler joined an attacker-controlled request field
//! straight onto the skills directory.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};

const MAX_LEN: usize = 64;

/// A skill's directory name: `[a-z0-9][a-z0-9._-]{0,63}`, never `.` or `..`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct SkillName(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillNameError {
    Empty,
    TooLong,
    DotSegment,
    BadChar(char),
}

impl fmt::Display for SkillNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("skill name is empty"),
            Self::TooLong => write!(f, "skill name is longer than {MAX_LEN} characters"),
            Self::DotSegment => f.write_str("skill name may not be `.` or `..`"),
            Self::BadChar(c) => write!(f, "skill name may not contain `{c}`"),
        }
    }
}

impl std::error::Error for SkillNameError {}

impl SkillName {
    pub fn parse(raw: &str) -> Result<Self, SkillNameError> {
        let name = raw.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err(SkillNameError::Empty);
        }
        if name.len() > MAX_LEN {
            return Err(SkillNameError::TooLong);
        }
        if name == "." || name == ".." {
            return Err(SkillNameError::DotSegment);
        }
        if name.starts_with('.') {
            return Err(SkillNameError::BadChar('.'));
        }
        if let Some(bad) = name
            .chars()
            .find(|c| !matches!(c, 'a'..='z' | '0'..='9' | '.' | '_' | '-'))
        {
            return Err(SkillNameError::BadChar(bad));
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The only sanctioned way to turn a name into a path.
    pub fn dir_in(&self, root: &Path) -> PathBuf {
        root.join(&self.0)
    }
}

impl fmt::Display for SkillName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for SkillName {
    type Err = SkillNameError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw)
    }
}

/// Validated on the way in, so a request body can never carry an invalid name into a
/// handler that then has to remember to check it.
impl<'de> Deserialize<'de> for SkillName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "name_tests.rs"]
mod name_tests;
