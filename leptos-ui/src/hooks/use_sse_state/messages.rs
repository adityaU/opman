//! Message-related methods on SseState — loading, pagination, optimistic messages,
//! batched flush scheduling for both main session and subagent sessions.

use crate::sse::message_map::{self, MessageMap, map_to_sorted_array};
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::types::MESSAGE_PAGE_SIZE;
use super::SseState;

impl SseState {
    /// Update the message map with a closure and schedule a batched flush.
    /// Multiple updates within the same animation frame are coalesced into a single flush.
    pub fn update_message_map(&self, f: impl FnOnce(&mut MessageMap) -> bool) {
        let mut changed = false;
        self.message_map.update(|map: &mut MessageMap| {
            changed = f(map);
        });
        if changed {
            self.schedule_flush();
        }
    }

    /// Schedule a flush for the next animation frame if one isn't already pending.
    pub(super) fn schedule_flush(&self) {
        if self.flush_pending.get_untracked() {
            return; // Already scheduled
        }
        self.flush_pending.set(true);

        let message_map = self.message_map;
        let set_messages = self.set_messages;
        let flush_pending = self.flush_pending;
        let raf_handle = self.raf_handle;

        let cb = Closure::once(move || {
            flush_pending.set(false);
            raf_handle.set(0);
            let sorted = {
                let map = message_map.get_untracked();
                map_to_sorted_array(&map)
            };
            set_messages.set(sorted);
        });

        if let Some(window) = web_sys::window() {
            if let Ok(handle) = window.request_animation_frame(cb.as_ref().unchecked_ref()) {
                self.raf_handle.set(handle);
            }
        }
        cb.forget(); // Safe: called exactly once by rAF
    }

    /// Flush the message map into the messages signal immediately (for non-streaming paths).
    pub fn flush_messages(&self) {
        // Cancel any pending rAF flush to avoid double-flush
        if self.flush_pending.get_untracked() {
            self.flush_pending.set(false);
            let handle = self.raf_handle.get_untracked();
            if handle != 0 {
                if let Some(window) = web_sys::window() {
                    let _ = window.cancel_animation_frame(handle);
                }
                self.raf_handle.set(0);
            }
        }
        let sorted = {
            let map = self.message_map.get_untracked();
            map_to_sorted_array(&map)
        };
        self.set_messages.set(sorted);
    }

    /// Refresh messages from the server for the active session.
    /// Guarded: concurrent calls are skipped to prevent stacking fetches.
    pub fn refresh_messages(&self) {
        if self.is_refreshing.get_untracked() {
            return;
        }
        let sid = match self.tracked_session_id() {
            Some(s) => s,
            None => return,
        };
        self.is_refreshing.set(true);
        let gen = self.current_gen();
        let set_messages = self.set_messages;
        let set_has_older = self.set_has_older_messages;
        let set_total = self.set_total_message_count;
        let message_map_signal = self.message_map;
        let session_gen_signal = self.session_gen;
        let is_refreshing = self.is_refreshing;

        leptos::task::spawn_local(async move {
            let result =
                crate::api::fetch_session_messages(&sid, MESSAGE_PAGE_SIZE, None).await;
            is_refreshing.set(false);
            match result {
                Ok(resp) => {
                    if session_gen_signal.get_untracked() != gen {
                        return;
                    }
                    message_map_signal.update(|map: &mut MessageMap| {
                        // Purge stale optimistic entries now that real data is loaded
                        map.retain(|k, _| !k.starts_with("__optimistic__"));
                        for msg in resp.messages {
                            let id = message_map::effective_id(&msg.info);
                            if !id.is_empty() {
                                map.insert(id, msg);
                            }
                        }
                    });
                    let sorted = {
                        let map = message_map_signal.get_untracked();
                        map_to_sorted_array(&map)
                    };
                    set_messages.set(sorted);
                    set_has_older.set(resp.has_more);
                    set_total.set(resp.total);
                }
                Err(e) => {
                    log::error!("refreshMessages failed: {}", e);
                }
            }
        });
    }

    /// Load the active session's messages (full initial fetch, with loading indicator).
    pub fn load_session_messages(&self, session_id: String) {
        let gen = self.current_gen();
        let session_gen_signal = self.session_gen;
        let set_messages = self.set_messages;
        let set_loading = self.set_is_loading_messages;
        let set_has_older = self.set_has_older_messages;
        let set_total = self.set_total_message_count;
        let set_stats = self.set_stats;
        let message_map_signal = self.message_map;

        // Clear state
        message_map_signal.update(|map: &mut MessageMap| {
            map.clear();
        });
        set_messages.set(Vec::new());
        set_has_older.set(false);
        set_total.set(0);
        set_loading.set(true);

        let sid = session_id.clone();
        leptos::task::spawn_local(async move {
            let result =
                crate::api::fetch_session_messages(&sid, MESSAGE_PAGE_SIZE, None).await;
            if session_gen_signal.get_untracked() != gen {
                // Session switched while fetching — still clear loading for the old request.
                // The new session's load_session_messages will manage its own loading state.
                return;
            }
            match result {
                Ok(resp) => {
                    message_map_signal.update(|map: &mut MessageMap| {
                        for msg in resp.messages {
                            let id = message_map::effective_id(&msg.info);
                            if !id.is_empty() {
                                map.insert(id, msg);
                            }
                        }
                    });
                    let sorted = {
                        let map = message_map_signal.get_untracked();
                        map_to_sorted_array(&map)
                    };
                    set_messages.set(sorted);
                    set_has_older.set(resp.has_more);
                    set_total.set(resp.total);
                }
                Err(e) => {
                    log::error!("loadSessionMessages failed: {}", e);
                    set_messages.set(Vec::new());
                }
            }
            set_loading.set(false);
        });

        // Also fetch stats
        let sid2 = session_id;
        let gen2 = gen;
        leptos::task::spawn_local(async move {
            match crate::api::fetch_session_stats(&sid2).await {
                Ok(stats) => {
                    if session_gen_signal.get_untracked() == gen2 {
                        set_stats.set(Some(stats));
                    }
                }
                Err(_) => {}
            }
        });
    }

    /// Load older messages (pagination).
    pub fn load_older_messages(&self) {
        if self.is_loading_older.get_untracked() {
            return;
        }
        let sid = match self.tracked_session_id() {
            Some(s) => s,
            None => return,
        };
        let gen = self.current_gen();
        let session_gen_signal = self.session_gen;
        let message_map_signal = self.message_map;
        let set_loading = self.set_is_loading_older;
        let set_messages = self.set_messages;
        let set_has_older = self.set_has_older_messages;

        // Find oldest timestamp
        let oldest_ts = {
            let map = message_map_signal.get_untracked();
            let mut oldest = f64::INFINITY;
            for msg in map.values() {
                let ts = message_map::get_message_time(msg);
                if ts > 0.0 && ts < oldest {
                    oldest = ts;
                }
            }
            oldest
        };
        if oldest_ts == f64::INFINITY {
            return;
        }

        set_loading.set(true);

        leptos::task::spawn_local(async move {
            let result =
                crate::api::fetch_session_messages(&sid, MESSAGE_PAGE_SIZE, Some(oldest_ts))
                    .await;
            if session_gen_signal.get_untracked() != gen {
                set_loading.set(false);
                return;
            }
            match result {
                Ok(resp) => {
                    message_map_signal.update(|map: &mut MessageMap| {
                        for msg in resp.messages {
                            let id = message_map::effective_id(&msg.info);
                            if !id.is_empty() && !map.contains_key(&id) {
                                map.insert(id, msg);
                            }
                        }
                    });
                    let sorted = {
                        let map = message_map_signal.get_untracked();
                        map_to_sorted_array(&map)
                    };
                    set_messages.set(sorted);
                    set_has_older.set(resp.has_more);
                }
                Err(e) => {
                    log::error!("loadOlderMessages failed: {}", e);
                }
            }
            set_loading.set(false);
        });
    }

    /// Add an optimistic user message.
    pub fn add_optimistic_message(&self, text: &str) {
        let id = format!("__optimistic__{}", js_sys::Date::now() as u64);
        let sid = self.tracked_session_id();
        let msg = crate::types::core::Message {
            info: crate::types::core::MessageInfo {
                role: "user".to_string(),
                message_id: Some(id.clone()),
                id: Some(id.clone()),
                session_id: sid,
                time: Some(serde_json::Value::Number(
                    serde_json::Number::from_f64(js_sys::Date::now() / 1000.0)
                        .unwrap_or_else(|| serde_json::Number::from(0)),
                )),
                ..Default::default()
            },
            parts: vec![crate::types::core::MessagePart {
                part_type: "text".to_string(),
                text: Some(text.to_string()),
                ..Default::default()
            }],
            metadata: None,
        };
        let id_clone = id;
        self.message_map.update(|map: &mut MessageMap| {
            map.insert(id_clone, msg);
        });
        self.flush_messages();
    }
}
