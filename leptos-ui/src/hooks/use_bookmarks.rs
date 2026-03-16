//! Bookmarks hook — localStorage-persisted message bookmarks.
//! Matches React `useBookmarks.ts`.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const STORAGE_KEY: &str = "opman-bookmarks";

// ── Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub message_id: String,
    pub session_id: String,
    pub role: String,
    pub preview: String,
    pub created_at: f64,
}

/// Bookmark state returned by `use_bookmarks`.
#[derive(Clone, Copy)]
pub struct BookmarkState {
    bookmarks: RwSignal<HashMap<String, Bookmark>>,
}

impl BookmarkState {
    /// Check if a message is bookmarked.
    pub fn is_bookmarked(&self, message_id: &str) -> bool {
        self.bookmarks.get_untracked().contains_key(message_id)
    }

    /// Reactive check if a message is bookmarked.
    pub fn is_bookmarked_tracked(&self, message_id: String) -> impl Fn() -> bool {
        let bookmarks = self.bookmarks;
        move || bookmarks.get().contains_key(&message_id)
    }

    /// Toggle a bookmark.
    pub fn toggle_bookmark(&self, message_id: &str, session_id: &str, role: &str, preview: &str) {
        let mid = message_id.to_string();
        self.bookmarks.update(|map| {
            if map.contains_key(&mid) {
                map.remove(&mid);
            } else {
                let truncated = if preview.len() > 120 {
                    format!("{}...", &preview[..117])
                } else {
                    preview.to_string()
                };
                map.insert(
                    mid.clone(),
                    Bookmark {
                        message_id: mid,
                        session_id: session_id.to_string(),
                        role: role.to_string(),
                        preview: truncated,
                        created_at: js_sys::Date::now() / 1000.0,
                    },
                );
            }
        });
        self.persist();
    }

    /// Remove a bookmark.
    pub fn remove_bookmark(&self, message_id: &str) {
        self.bookmarks.update(|map| {
            map.remove(message_id);
        });
        self.persist();
    }

    /// Get all bookmarks sorted by created_at descending.
    pub fn all_bookmarks(&self) -> Vec<Bookmark> {
        let map = self.bookmarks.get_untracked();
        let mut list: Vec<Bookmark> = map.values().cloned().collect();
        list.sort_by(|a, b| {
            b.created_at
                .partial_cmp(&a.created_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list
    }

    /// Get bookmarks for a specific session.
    pub fn get_session_bookmarks(&self, session_id: &str) -> Vec<Bookmark> {
        let map = self.bookmarks.get_untracked();
        let mut list: Vec<Bookmark> = map
            .values()
            .filter(|b| b.session_id == session_id)
            .cloned()
            .collect();
        list.sort_by(|a, b| {
            b.created_at
                .partial_cmp(&a.created_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list
    }

    /// Persist to localStorage.
    fn persist(&self) {
        let map = self.bookmarks.get_untracked();
        let list: Vec<&Bookmark> = map.values().collect();
        if let Ok(json) = serde_json::to_string(&list) {
            if let Some(storage) = web_sys::window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
            {
                let _ = storage.set_item(STORAGE_KEY, &json);
            }
        }
    }

    /// Load from localStorage.
    fn load() -> HashMap<String, Bookmark> {
        let json = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten()
            .and_then(|s| s.get_item(STORAGE_KEY).ok())
            .flatten();
        match json {
            Some(data) => {
                let list: Vec<Bookmark> = serde_json::from_str(&data).unwrap_or_default();
                list.into_iter()
                    .map(|b| (b.message_id.clone(), b))
                    .collect()
            }
            None => HashMap::new(),
        }
    }
}

// ── Hook ───────────────────────────────────────────────────────────

/// Create bookmark state. Call once at layout level.
pub fn use_bookmarks() -> BookmarkState {
    let initial = BookmarkState::load();
    let bookmarks = RwSignal::new(initial);
    BookmarkState { bookmarks }
}
