//! Subagent message methods and recovery helpers on SseState.
//! Subagent sessions have their own per-session message maps, flushed separately.
//!
//! Dirty-tracking: only sessions whose maps actually changed are re-rendered
//! during a flush, avoiding O(N) Vec rebuilds for every rAF tick.

use crate::sse::message_map::{MessageMap, map_to_sorted_array};
use leptos::prelude::*;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::SseState;

impl SseState {
    /// Update a subagent session's message map and schedule a batched subagent flush.
    pub fn update_subagent_map(
        &self,
        session_id: &str,
        f: impl FnOnce(&mut MessageMap) -> bool,
    ) {
        let sid = session_id.to_string();
        let mut changed = false;
        self.subagent_maps.update(|maps| {
            let map = maps.entry(sid.clone()).or_insert_with(MessageMap::new);
            changed = f(map);
        });
        if changed {
            self.subagent_dirty.update(|d| { d.insert(sid); });
            self.schedule_subagent_flush();
        }
    }

    /// Schedule a subagent flush for the next animation frame.
    /// Only re-renders sessions marked dirty since the last flush.
    fn schedule_subagent_flush(&self) {
        if self.subagent_flush_pending.get_untracked() {
            return;
        }
        self.subagent_flush_pending.set(true);

        let subagent_maps = self.subagent_maps;
        let set_subagent_messages = self.set_subagent_messages;
        let subagent_flush_pending = self.subagent_flush_pending;
        let subagent_raf_handle = self.subagent_raf_handle;
        let subagent_dirty = self.subagent_dirty;

        let cb = Closure::once(move || {
            subagent_flush_pending.set(false);
            subagent_raf_handle.set(0);

            // Take the dirty set — only these sessions need re-rendering.
            let dirty: Vec<String> = subagent_dirty.with_untracked(|d| d.iter().cloned().collect());
            subagent_dirty.update(|d| d.clear());

            if dirty.is_empty() {
                return;
            }

            // Build rendered arrays only for dirty sessions.
            let patches: Vec<(String, Vec<crate::types::core::Message>)> =
                subagent_maps.with_untracked(|maps| {
                    dirty.iter().filter_map(|sid| {
                        maps.get(sid).map(|map| (sid.clone(), map_to_sorted_array(map)))
                    }).collect()
                });

            // Patch into existing HashMap — untouched sessions keep their
            // exact same Vec allocation, so derived signals that read those
            // sessions will not re-fire.
            set_subagent_messages.update(|current| {
                for (sid, msgs) in patches {
                    current.insert(sid, msgs);
                }
            });
        });

        if let Some(window) = web_sys::window() {
            if let Ok(handle) =
                window.request_animation_frame(cb.as_ref().unchecked_ref())
            {
                self.subagent_raf_handle.set(handle);
            }
        }
        cb.forget();
    }

    /// Clear all subagent message maps (e.g. on session switch).
    pub fn clear_subagent_messages(&self) {
        // Cancel pending subagent rAF
        if self.subagent_flush_pending.get_untracked() {
            self.subagent_flush_pending.set(false);
            let handle = self.subagent_raf_handle.get_untracked();
            if handle != 0 {
                if let Some(window) = web_sys::window() {
                    let _ = window.cancel_animation_frame(handle);
                }
                self.subagent_raf_handle.set(0);
            }
        }
        self.subagent_dirty.update(|d| d.clear());
        self.subagent_maps.update(|maps| maps.clear());
        self.set_subagent_messages.set(HashMap::new());
    }

    /// Hydrate pending permissions and questions from the server.
    /// Called during reconnect recovery to catch any events missed while disconnected.
    pub fn hydrate_pending(&self) {
        let set_permissions = self.set_permissions;
        let set_questions = self.set_questions;
        leptos::task::spawn_local(async move {
            match crate::api::fetch_pending().await {
                Ok(pending) => {
                    use crate::sse::connection::session_handlers::{
                        parse_permission_from_props, parse_question_from_props,
                    };
                    let perms: Vec<crate::types::core::PermissionRequest> = pending
                        .permissions
                        .iter()
                        .filter_map(|v| parse_permission_from_props(v))
                        .collect();
                    let qs: Vec<crate::types::core::QuestionRequest> = pending
                        .questions
                        .iter()
                        .filter_map(|v| parse_question_from_props(v))
                        .collect();
                    if !perms.is_empty() {
                        set_permissions.update(
                            move |prev: &mut Vec<crate::types::core::PermissionRequest>| {
                                let ids: std::collections::HashSet<String> =
                                    prev.iter().map(|p| p.id.clone()).collect();
                                for p in perms {
                                    if !ids.contains(&p.id) {
                                        prev.push(p);
                                    }
                                }
                            },
                        );
                    }
                    if !qs.is_empty() {
                        set_questions.update(
                            move |prev: &mut Vec<crate::types::core::QuestionRequest>| {
                                let ids: std::collections::HashSet<String> =
                                    prev.iter().map(|q| q.id.clone()).collect();
                                for q in qs {
                                    if !ids.contains(&q.id) {
                                        prev.push(q);
                                    }
                                }
                            },
                        );
                    }
                }
                Err(e) => log::error!("hydratePending failed: {}", e),
            }
        });
    }
}
