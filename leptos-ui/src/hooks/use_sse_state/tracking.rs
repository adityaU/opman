//! Active session tracking — session-switch guard logic.
//! Called from a Memo-driven Effect in chat_layout.

use crate::sse::message_map::MessageMap;
use leptos::prelude::*;

use super::types::SessionStatus;
use super::SseState;

impl SseState {
    /// Track the active session — called from a Memo-driven Effect in chat_layout.
    /// Uses **untracked** reads — the caller (Memo) handles reactivity.
    /// Implements the session-switch guard logic from React useSSE.
    pub fn track_active_session(&self) {
        let state = self.app_state.get_untracked();
        let state = match state.as_ref() {
            Some(s) => s,
            None => return,
        };
        let proj = match state.projects.get(state.active_project) {
            Some(p) => p,
            None => return,
        };
        let new_sid = proj.active_session.as_deref();
        let current_sid = self.active_session_id_stored.get_untracked();

        if new_sid.map(|s| s.to_string()) == current_sid {
            return; // No change
        }

        // Guard: if we already have an active session and the change wasn't user-initiated,
        // ignore the server's active_session to prevent unwanted switches.
        let expect = self.expect_switch.get_untracked();
        if current_sid.is_some() && !expect && new_sid.is_some() {
            return;
        }
        self.expect_switch.set(false);

        // Increment session generation
        let new_gen = self.session_gen.get_untracked() + 1;
        self.session_gen.set(new_gen);

        // Update tracked session
        let new_sid_owned = new_sid.map(|s| s.to_string());
        self.active_session_id_stored.set(new_sid_owned.clone());

        // Immediately recompute session status from busy_sessions
        let busy = self.busy_sessions.get_untracked();
        if let Some(ref sid) = new_sid_owned {
            if busy.contains(sid) {
                self.set_session_status.set(SessionStatus::Busy);
            } else {
                self.set_session_status.set(SessionStatus::Idle);
            }
        } else {
            self.set_session_status.set(SessionStatus::Idle);
        }

        // Load messages for the new session
        if let Some(sid) = new_sid_owned {
            self.clear_subagent_messages();
            self.load_session_messages(sid);
        } else {
            // No active session — clear messages
            self.message_map.update(|map: &mut MessageMap| {
                map.clear();
            });
            self.set_messages.set(Vec::new());
            self.set_has_older_messages.set(false);
            self.set_total_message_count.set(0);
            self.set_is_loading_messages.set(false);
            self.clear_subagent_messages();
        }
    }
}
