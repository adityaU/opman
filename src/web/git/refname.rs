//! Validated newtypes for the untrusted strings that reach a `git` argv.
//!
//! Every value that crosses into [`crate::web::git::exec`] arrives as one of
//! these, so a handler cannot forget to validate: there is no way to build one
//! except through the checked constructor.

use std::borrow::Cow;

use crate::web::error::{WebError, WebResult};

/// A branch, tag or symbolic ref name that is safe to place in an argv.
///
/// Rejects the shapes git would reinterpret: leading `-` (an option), range
/// syntax (`..`), and the revision operators `~ ^ :`. A `/` is allowed because
/// remote-tracking names legitimately contain one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefName<'a>(&'a str);

impl<'a> RefName<'a> {
    pub fn parse(raw: &'a str) -> WebResult<Self> {
        let bad = raw.is_empty()
            || raw.starts_with('-')
            || raw.contains("..")
            || raw.contains(['~', '^', ':', '?', '*', '[', '\\', '\n'])
            || raw.ends_with('/')
            || raw.ends_with(".lock");
        if bad {
            return Err(WebError::BadRequest(format!("Invalid git ref name: {raw}")));
        }
        Ok(Self(raw))
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }

    /// The remote name, when this ref is a remote-tracking name that the given
    /// remotes list knows about — `origin/feat/x` with `origin` present yields
    /// `("origin", "feat/x")`.
    ///
    /// Matching against the real remotes rather than splitting on the first `/`
    /// is what keeps a local branch called `feature/login` from being read as
    /// remote `feature`.
    pub fn split_remote(self, remotes: &[String]) -> Option<(&'a str, &'a str)> {
        remotes.iter().find_map(|remote| {
            let rest = self.0.strip_prefix(remote.as_str())?.strip_prefix('/')?;
            (!rest.is_empty()).then(|| (&self.0[..remote.len()], rest))
        })
    }
}

/// A revision expression: anything `git rev-parse` accepts for a single
/// commit, such as `HEAD~1`, `main^`, `origin/main@{2}` or a raw hash.
///
/// [`RefName`] deliberately rejects `~ ^ @{}` because a *branch* may not
/// contain them, but a reset or a diff target legitimately may. The safety
/// property that matters at an argv boundary is narrower: it must not be
/// readable as an option, and it must be one token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revision<'a>(&'a str);

impl<'a> Revision<'a> {
    pub fn parse(raw: &'a str) -> WebResult<Self> {
        let bad = raw.is_empty()
            || raw.starts_with('-')
            || raw.contains("..")
            || raw.contains(':')
            || raw.contains(['?', '*', '[', '\\', '\n', '\r', '\0'])
            || raw.chars().any(char::is_whitespace);
        if bad {
            return Err(WebError::BadRequest(format!("Invalid revision: {raw}")));
        }
        Ok(Self(raw))
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }
}

/// A commit-ish hash. Hex only, so it can never be read as an option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitHash<'a>(&'a str);

impl<'a> CommitHash<'a> {
    pub fn parse(raw: &'a str) -> WebResult<Self> {
        if raw.is_empty() || raw.len() > 64 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(WebError::BadRequest("Invalid git hash".into()));
        }
        Ok(Self(raw))
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }
}

/// A repository-relative path safe to pass after a `--` separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoPath<'a>(&'a str);

impl<'a> RepoPath<'a> {
    pub fn parse(raw: &'a str) -> WebResult<Self> {
        if raw.is_empty() || raw.starts_with('-') || raw.contains('\0') {
            return Err(WebError::BadRequest("Invalid path".into()));
        }
        Ok(Self(raw))
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }
}

/// A stash entry reference such as `stash@{2}`.
///
/// `{` and `}` are legal here but nowhere else, so this gets its own type
/// rather than a relaxed [`RefName`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StashRef<'a>(&'a str);

impl<'a> StashRef<'a> {
    pub fn parse(raw: &'a str) -> WebResult<Self> {
        let shaped = raw
            .strip_prefix("stash@{")
            .and_then(|rest| rest.strip_suffix('}'))
            .is_some_and(|idx| !idx.is_empty() && idx.chars().all(|c| c.is_ascii_digit()));
        if !shaped {
            return Err(WebError::BadRequest(format!("Invalid stash ref: {raw}")));
        }
        Ok(Self(raw))
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }
}

/// A free-text message (commit subject, stash label) reaching argv after `-m`.
///
/// Nothing about the content is dangerous once it is a distinct argv entry;
/// only an empty message and interior NULs are rejected.
pub fn message(raw: &str) -> WebResult<Cow<'_, str>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(WebError::BadRequest("Message cannot be empty".into()));
    }
    if trimmed.contains('\0') {
        return Err(WebError::BadRequest("Message contains a NUL byte".into()));
    }
    Ok(Cow::Borrowed(trimmed))
}

#[cfg(test)]
#[path = "refname_tests.rs"]
mod refname_tests;
