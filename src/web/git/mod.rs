//! Git plumbing shared by every `/api/git/*` handler.
//!
//! `exec` owns process spawning, `refname` owns argv validation, and `scope`
//! owns turning a request's `repo` field into a directory. Handlers compose
//! these; none of them build a [`tokio::process::Command`] themselves.

pub mod exec;
pub mod refname;
pub mod scope;

pub use exec::{GitFailure, GitOutput, GitRefusal, GitResult, Reach};
pub use refname::{CommitHash, RefName, RepoPath, Revision, StashRef};
