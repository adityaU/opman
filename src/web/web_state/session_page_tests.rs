use std::collections::HashSet;

use super::*;
use crate::app::{SessionInfo, SessionTime};

fn session(id: &str, parent: &str, updated: u64) -> SessionInfo {
    SessionInfo {
        id: id.to_string(),
        title: id.to_string(),
        parent_id: parent.to_string(),
        time: SessionTime {
            created: updated,
            updated,
        },
        ..SessionInfo::default()
    }
}

/// Ten parents, newest first, each `s{n}` updated at `n`.
fn ten() -> Vec<SessionInfo> {
    (0..10).map(|n| session(&format!("s{n}"), "", n)).collect()
}

fn ids(slicing: &SessionSlicing<'_>) -> Vec<String> {
    slicing.sessions.iter().map(|s| s.id.clone()).collect()
}

#[test]
fn first_page_is_the_newest_and_total_counts_every_parent() {
    let all = ten();
    let out = slice_sessions(
        &all,
        SessionSlice::Page {
            offset: 0,
            limit: 3,
        },
        &HashSet::new(),
    );
    assert_eq!(ids(&out), ["s9", "s8", "s7"]);
    assert_eq!(out.total, 10);
}

#[test]
fn offset_walks_further_back_without_changing_the_total() {
    let all = ten();
    let out = slice_sessions(
        &all,
        SessionSlice::Page {
            offset: 3,
            limit: 3,
        },
        &HashSet::new(),
    );
    assert_eq!(ids(&out), ["s6", "s5", "s4"]);
    assert_eq!(out.total, 10);
}

#[test]
fn a_page_past_the_end_is_empty_rather_than_an_error() {
    let all = ten();
    let out = slice_sessions(
        &all,
        SessionSlice::Page {
            offset: 50,
            limit: 20,
        },
        &HashSet::new(),
    );
    assert!(out.sessions.is_empty());
    assert_eq!(out.total, 10);
}

#[test]
fn subagents_ride_along_with_their_parent_and_never_consume_the_page() {
    let mut all = ten();
    all.push(session("kid", "s9", 99));
    all.push(session("orphan", "s0", 99));
    let out = slice_sessions(
        &all,
        SessionSlice::Page {
            offset: 0,
            limit: 2,
        },
        &HashSet::new(),
    );
    // s0's child is left behind with s0; the page is still two parents wide.
    assert_eq!(ids(&out), ["s9", "s8", "kid"]);
    assert_eq!(out.total, 10);
}

#[test]
fn a_pinned_session_survives_falling_off_the_first_page() {
    let all = ten();
    let pinned = HashSet::from(["s1"]);
    let out = slice_sessions(
        &all,
        SessionSlice::Page {
            offset: 0,
            limit: 2,
        },
        &pinned,
    );
    assert_eq!(ids(&out), ["s9", "s8", "s1"]);
}

#[test]
fn a_pinned_session_already_on_the_page_is_not_sent_twice() {
    let all = ten();
    let pinned = HashSet::from(["s9"]);
    let out = slice_sessions(
        &all,
        SessionSlice::Page {
            offset: 0,
            limit: 2,
        },
        &pinned,
    );
    assert_eq!(ids(&out), ["s9", "s8"]);
}

#[test]
fn pins_are_ignored_past_the_first_page_because_the_client_already_holds_them() {
    let all = ten();
    let pinned = HashSet::from(["s9"]);
    let out = slice_sessions(
        &all,
        SessionSlice::Page {
            offset: 2,
            limit: 2,
        },
        &pinned,
    );
    assert_eq!(ids(&out), ["s7", "s6"]);
}

#[test]
fn an_id_lookup_returns_those_sessions_with_their_children_whatever_their_age() {
    let mut all = ten();
    all.push(session("kid", "s0", 5));
    let wanted = vec!["s0".to_string(), "missing".to_string()];
    let out = slice_sessions(&all, SessionSlice::Ids(&wanted), &HashSet::new());
    assert_eq!(ids(&out), ["s0", "kid"]);
    assert_eq!(out.total, 10);
}
