//! Which slice of a project's session list goes over the wire.
//!
//! The full list stays in memory — status polling, runner bindings and
//! active-session repair all need every session. What is bounded is the payload:
//! `/api/state` ships the newest page and a total, and the sidebar asks for the
//! next page only when the user opens one.
//!
//! Paging is over *top-level* sessions. Subagents ride along with the parent that
//! owns them, so a page never splits a session from its children.

use std::collections::HashSet;

use crate::app::SessionInfo;

/// Top-level sessions per page. Also the size of the first page in `/api/state`.
pub(crate) const SESSION_PAGE: usize = 20;

/// What a caller wants out of a project's session list.
pub(crate) enum SessionSlice<'a> {
    /// The newest top-level sessions after `offset`, plus their subagents.
    Page { offset: usize, limit: usize },
    /// Named sessions only, whatever their age. Used for client-side state the
    /// server does not know about — pinned rows, open tabs — so those survive
    /// falling off the first page.
    Ids(&'a [String]),
}

/// A page of sessions and how many top-level sessions exist in total.
pub(crate) struct SessionSlicing<'a> {
    pub(crate) sessions: Vec<&'a SessionInfo>,
    pub(crate) total: usize,
}

fn is_parent(session: &SessionInfo) -> bool {
    session.parent_id.is_empty()
}

/// Take `slice` out of `sessions`, keeping `pinned` ids regardless of age.
///
/// `pinned` is only honoured on the first page: later pages are appended to a map
/// the client already holds, so re-sending them would be pure duplication.
pub(crate) fn slice_sessions<'a>(
    sessions: &'a [SessionInfo],
    slice: SessionSlice<'_>,
    pinned: &HashSet<&str>,
) -> SessionSlicing<'a> {
    let total = sessions.iter().filter(|s| is_parent(s)).count();

    let (offset, limit) = match slice {
        SessionSlice::Ids(ids) => {
            let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
            let picked = sessions
                .iter()
                .filter(|s| wanted.contains(s.id.as_str()))
                .collect();
            return with_children(sessions, picked, total);
        }
        SessionSlice::Page { offset, limit } => (offset, limit),
    };

    let mut parents: Vec<&SessionInfo> = sessions.iter().filter(|s| is_parent(s)).collect();
    parents.sort_unstable_by(|a, b| b.time.updated.cmp(&a.time.updated));

    let mut picked: Vec<&SessionInfo> = parents.iter().skip(offset).take(limit).copied().collect();
    if offset == 0 && !pinned.is_empty() {
        let taken: HashSet<&str> = picked.iter().map(|s| s.id.as_str()).collect();
        let extra: Vec<&SessionInfo> = parents
            .iter()
            .filter(|s| pinned.contains(s.id.as_str()) && !taken.contains(s.id.as_str()))
            .copied()
            .collect();
        picked.extend(extra);
    }

    with_children(sessions, picked, total)
}

/// Append every subagent whose parent is in `picked`.
fn with_children<'a>(
    sessions: &'a [SessionInfo],
    mut picked: Vec<&'a SessionInfo>,
    total: usize,
) -> SessionSlicing<'a> {
    let parents: HashSet<&str> = picked.iter().map(|s| s.id.as_str()).collect();
    let children: Vec<&SessionInfo> = sessions
        .iter()
        .filter(|s| !is_parent(s) && parents.contains(s.parent_id.as_str()))
        .collect();
    picked.extend(children);
    SessionSlicing {
        sessions: picked,
        total,
    }
}

#[cfg(test)]
#[path = "session_page_tests.rs"]
mod session_page_tests;
