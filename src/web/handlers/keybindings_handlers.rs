//! Read and write the web UI keybinding config.
//!
//! Validation of chords and command ids stays in the web UI, which owns the
//! command registry; this pair only guarantees a well-formed file on disk.

use axum::extract::Json;
use axum::response::IntoResponse;

use super::super::auth::AuthUser;
use super::super::error::WebError;
use super::super::keybindings::{self, KeybindingsConfig};

/// GET /api/keybindings
pub async fn get_keybindings(_auth: AuthUser) -> impl IntoResponse {
    Json(keybindings::load())
}

/// PUT /api/keybindings
///
/// Replaces the file wholesale: the keybindings view always sends the complete
/// config, so a partial merge here would only be a second, divergent source of
/// truth for what the user's keymap is.
pub async fn put_keybindings(
    _auth: AuthUser,
    Json(config): Json<KeybindingsConfig>,
) -> Result<impl IntoResponse, WebError> {
    keybindings::save(&config)
        .map_err(|err| WebError::Internal(format!("could not save keybindings.json: {err}")))?;
    Ok(Json(keybindings::load()))
}
