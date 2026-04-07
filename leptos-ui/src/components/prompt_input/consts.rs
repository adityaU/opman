//! Shared constants, helpers, and types for the prompt_input module.

// ── Constants ───────────────────────────────────────────────────────

pub(crate) const ACCEPTED_IMAGE_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/svg+xml",
    "image/bmp",
];
pub(crate) const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024;

pub(crate) const NO_ARG_COMMANDS: &[&str] = &[
    "new",
    "cancel",
    "compact",
    "copy",
    "undo",
    "redo",
    "share",
    "fork",
    "terminal",
    "clear",
    "models",
    "keys",
    "keybindings",
    "todos",
    "sessions",
    "context",
    "settings",
    "assistant-center",
    "inbox",
    "missions",
    "memory",
    "autonomy",
    "routines",
    "delegation",
    "workspaces",
    "system",
];

pub(super) const SEARCH_PREVIEW_LEN: usize = 80;

// ── Helpers ─────────────────────────────────────────────────────────

/// Truncate a model ID to a short display name.
pub(super) fn short_model_name(model_id: &str) -> String {
    let parts: Vec<&str> = model_id.split('/').collect();
    let name = parts.last().unwrap_or(&model_id);
    if name.chars().count() > 30 {
        format!("{}...", name.chars().take(28).collect::<String>())
    } else {
        name.to_string()
    }
}

/// Truncate a history entry to a preview string for reverse-search display.
pub(super) fn truncate_preview(s: &str) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > SEARCH_PREVIEW_LEN {
        format!("{}...", &first_line[..SEARCH_PREVIEW_LEN])
    } else {
        first_line.to_string()
    }
}

// ── Types ───────────────────────────────────────────────────────────

/// Stored image attachment (base64 data-URL + metadata).
#[derive(Clone, Debug)]
pub(crate) struct ImageAttachmentLocal {
    pub data_url: String,
    pub name: String,
    pub mime_type: String,
}
