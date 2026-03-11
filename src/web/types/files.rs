//! File browsing, reading, writing, and editor LSP types.

use serde::{Deserialize, Serialize};

// ── File browsing types ─────────────────────────────────────────────

/// Query params for `GET /api/files?path=...`.
#[derive(Deserialize)]
pub struct FileBrowseQuery {
    /// Path relative to project root (default: ".")
    #[serde(default)]
    pub path: String,
}

/// A single directory entry.
#[derive(Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    /// File size in bytes (0 for directories).
    pub size: u64,
}

/// Response for `GET /api/files`.
#[derive(Serialize)]
pub struct FileBrowseResponse {
    pub path: String,
    pub entries: Vec<FileEntry>,
}

/// Query params for `GET /api/file/read?path=...`.
#[derive(Deserialize)]
pub struct FileReadQuery {
    /// Path relative to project root.
    pub path: String,
}

/// Response for `GET /api/file/read`.
#[derive(Serialize)]
pub struct FileReadResponse {
    pub path: String,
    pub content: String,
    /// Detected language hint (e.g. "rust", "javascript", "python").
    pub language: String,
}

/// Request body for `POST /api/file/write`.
#[derive(Deserialize)]
pub struct FileWriteRequest {
    /// Path relative to project root.
    pub path: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct EditorLspQuery {
    pub path: String,
    pub session_id: String,
    pub line: Option<i64>,
    pub col: Option<i64>,
}

#[derive(Deserialize)]
pub struct EditorFormatRequest {
    pub path: String,
    pub session_id: String,
}

// ── File Edit / Diff Review types ───────────────────────────────────

/// A single file edit event tracked during a session.
#[derive(Serialize, Clone, Debug)]
pub struct FileEditEntry {
    /// File path (relative to project root).
    pub path: String,
    /// Content before the edit (snapshot taken on first edit).
    pub original_content: String,
    /// Content after the edit (current file content at time of event).
    pub new_content: String,
    /// ISO 8601 timestamp of the edit event.
    pub timestamp: String,
    /// Sequential edit index (for ordering).
    pub index: usize,
}

/// Response for `GET /api/session/{id}/file-edits`.
#[derive(Serialize)]
pub struct FileEditsResponse {
    pub session_id: String,
    /// All file edits tracked for this session, ordered by time.
    pub edits: Vec<FileEditEntry>,
    /// Total number of files edited.
    pub file_count: usize,
}
