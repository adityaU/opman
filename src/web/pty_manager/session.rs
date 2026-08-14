//! A live web PTY as the shell picker sees it.

use std::path::PathBuf;

use serde::Serialize;

use super::activity::PtyActivity;
use super::kind::PtyKind;

/// What travels with a PTY for its whole life, independent of its bytes.
pub(crate) struct PtyMeta {
    pub(crate) kind: PtyKind,
    pub(crate) label: String,
    pub(crate) project: PathBuf,
}

/// One running PTY, described for a client that has to choose between them.
///
/// Carries `activity` so that listing the shells and asking which are busy is
/// one request. They were two endpoints over the same map, which meant the
/// picker could show a shell the activity poll had already dropped.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct PtySession {
    pub id: String,
    pub kind: PtyKind,
    pub label: String,
    /// Absolute path of the project the PTY was started in.
    pub project: String,
    pub activity: PtyActivity,
}

impl PtySession {
    pub(crate) fn new(id: &str, meta: &PtyMeta, activity: PtyActivity) -> Self {
        Self {
            id: id.to_owned(),
            kind: meta.kind,
            label: meta.label.clone(),
            project: meta.project.to_string_lossy().into_owned(),
            activity,
        }
    }
}
