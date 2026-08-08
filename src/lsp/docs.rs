//! Telling the server what the file says.
//!
//! A language server answers hover, definition and formatting only for
//! documents it has been told about. Every operation therefore begins by making
//! sure the document is open and current — but "current" has to be cheap,
//! because it runs on every hover, and correct, because a server whose text has
//! drifted from ours answers confidently about the wrong offsets.
//!
//! So: `didOpen` once, then `didChange` only when the text actually moved. When
//! the caller hands us the editor's live buffer we compare against what we last
//! sent; otherwise a `stat` decides, which costs microseconds against the
//! milliseconds a full read would.
//!
//! Sync is always full-document. Incremental ranges buy nothing when the input
//! is a whole buffer and are the single richest source of desync bugs.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::Result;
use serde_json::json;

use super::convert::path_to_uri;
use super::detect::LanguageId;
use super::peer::Peer;

/// What we last told the server about one document.
struct DocState {
    version: i64,
    /// Hash of the text we sent, so a live buffer can be compared exactly.
    hash: u64,
    /// Disk identity at that moment, for the no-content fast path.
    stamp: Option<(SystemTime, u64)>,
}

#[derive(Default)]
pub struct DocStore {
    open: Mutex<HashMap<String, DocState>>,
}

impl DocStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensure the server's copy of `path` matches `content` (or the file on
    /// disk when `content` is `None`). Sends at most one `didOpen` or
    /// `didChange`, and usually neither.
    pub fn sync(
        &self,
        peer: &Peer,
        path: &Path,
        language: LanguageId,
        content: Option<&str>,
    ) -> Result<()> {
        let uri = path_to_uri(path);
        let stamp = disk_stamp(path);

        // Decide without reading the file when we can.
        {
            let open = self.open.lock().map_err(lock_poisoned)?;
            if let Some(state) = open.get(&uri) {
                match content {
                    Some(text) if hash_of(text) == state.hash => return Ok(()),
                    None if stamp.is_some() && stamp == state.stamp => return Ok(()),
                    _ => {}
                }
            }
        }

        let text = match content {
            Some(text) => text.to_string(),
            None => std::fs::read_to_string(path)?,
        };
        let hash = hash_of(&text);

        let mut open = self.open.lock().map_err(lock_poisoned)?;
        match open.get_mut(&uri) {
            Some(state) => {
                // A second caller may have synced identical text while we read.
                if state.hash == hash {
                    state.stamp = stamp;
                    return Ok(());
                }
                state.version += 1;
                state.hash = hash;
                state.stamp = stamp;
                peer.notify(
                    "textDocument/didChange",
                    json!({
                        "textDocument": { "uri": uri, "version": state.version },
                        "contentChanges": [{ "text": text }],
                    }),
                )?;
            }
            None => {
                open.insert(
                    uri.clone(),
                    DocState {
                        version: 1,
                        hash,
                        stamp,
                    },
                );
                peer.notify(
                    "textDocument/didOpen",
                    json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": language,
                            "version": 1,
                            "text": text,
                        },
                    }),
                )?;
            }
        }
        Ok(())
    }

    /// Tell the server the file was written, for the many servers that only
    /// recompute on save.
    pub fn notify_saved(&self, peer: &Peer, path: &Path) {
        let uri = path_to_uri(path);
        if self
            .open
            .lock()
            .map(|o| !o.contains_key(&uri))
            .unwrap_or(true)
        {
            return;
        }
        let _ = peer.notify(
            "textDocument/didSave",
            json!({ "textDocument": { "uri": uri } }),
        );
    }

    /// Whether this document has ever been opened — the signal for "wait for a
    /// first publish" versus "report what we have".
    pub fn is_open(&self, uri: &str) -> bool {
        self.open
            .lock()
            .map(|open| open.contains_key(uri))
            .unwrap_or(false)
    }

    /// Forget everything. Used when the server is replaced: the new process has
    /// no documents, so our version numbers must restart too.
    pub fn clear(&self) {
        if let Ok(mut open) = self.open.lock() {
            open.clear();
        }
    }
}

fn disk_stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

fn hash_of(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn lock_poisoned<T>(_: T) -> anyhow::Error {
    anyhow::anyhow!("lsp document registry poisoned")
}

#[cfg(test)]
#[path = "docs_tests.rs"]
mod docs_tests;
