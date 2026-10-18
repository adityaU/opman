//! Release versions and the platforms we ship binaries for.

use std::fmt;
use std::str::FromStr;

/// A `major.minor.patch` release. Field order makes the derived `Ord` the release order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct VersionError(String);

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid version '{}' (expected e.g. 0.10.2 or v0.10.2)",
            self.0
        )
    }
}

impl std::error::Error for VersionError {}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || VersionError(s.to_owned());
        let text = s.trim();
        let mut parts = text.strip_prefix('v').unwrap_or(text).split('.');
        let mut next = || {
            let part = parts.next().ok_or_else(bad)?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            part.parse::<u64>().map_err(|_| bad())
        };
        let version = Self {
            major: next()?,
            minor: next()?,
            patch: next()?,
        };
        if parts.next().is_some() {
            return Err(bad());
        }
        Ok(version)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Version {
    /// The version this binary was built as.
    pub(crate) fn current() -> Option<Self> {
        env!("CARGO_PKG_VERSION").parse().ok()
    }

    /// The git tag / release name, e.g. `v0.10.2`.
    pub(crate) fn tag(&self) -> String {
        format!("v{self}")
    }
}

/// A platform the release workflow builds a binary for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    LinuxX64,
    LinuxArm64,
    MacX64,
    MacArm64,
}

impl Target {
    pub(crate) fn from_parts(os: &str, arch: &str) -> Option<Self> {
        match (os, arch) {
            ("linux", "x86_64") => Some(Self::LinuxX64),
            ("linux", "aarch64") => Some(Self::LinuxArm64),
            ("macos", "x86_64") => Some(Self::MacX64),
            ("macos", "aarch64") => Some(Self::MacArm64),
            _ => None,
        }
    }

    pub(crate) fn detect() -> Option<Self> {
        Self::from_parts(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// The release asset holding the binary, as named by `release.yml`.
    pub(crate) fn asset(self) -> &'static str {
        match self {
            Self::LinuxX64 => "opman-x86_64-unknown-linux-musl.tar.gz",
            Self::LinuxArm64 => "opman-aarch64-unknown-linux-musl.tar.gz",
            Self::MacX64 => "opman-x86_64-apple-darwin.tar.gz",
            Self::MacArm64 => "opman-aarch64-apple-darwin.tar.gz",
        }
    }
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
