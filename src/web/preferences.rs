//! Shared appearance preferences, also used by authenticated management tools.
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tokio::sync::Mutex;

use crate::web::{
    auth::AuthUser,
    error::{WebError, WebResult},
    types::{ServerState, WebEvent},
};

static SAVE: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    System,
    Light,
    Dark,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Glassy,
    Flat,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    Compact,
    Default,
    Roomy,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    appearance: Option<Appearance>,
    theme_mode: Option<ThemeMode>,
    density: Option<Density>,
}

fn path() -> WebResult<std::path::PathBuf> {
    dirs::config_dir()
        .map(|dir| dir.join("opman/preferences.json"))
        .ok_or_else(|| WebError::Internal("No config directory".into()))
}

async fn read(path: &std::path::Path) -> WebResult<Preferences> {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|error| WebError::Internal(error.to_string()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(error) => Err(WebError::Internal(error.to_string())),
    }
}

pub async fn get(_auth: AuthUser) -> WebResult<Json<Preferences>> {
    Ok(Json(read(&path()?).await?))
}

pub async fn put(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(patch): Json<Preferences>,
) -> WebResult<Json<Preferences>> {
    let _guard = SAVE.lock().await;
    let path = path()?;
    let mut saved = read(&path).await?;
    saved.appearance = patch.appearance.or(saved.appearance);
    saved.theme_mode = patch.theme_mode.or(saved.theme_mode);
    saved.density = patch.density.or(saved.density);
    let bytes =
        serde_json::to_vec_pretty(&saved).map_err(|error| WebError::Internal(error.to_string()))?;
    let parent = path
        .parent()
        .ok_or_else(|| WebError::Internal("No preferences directory".into()))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| WebError::Internal(error.to_string()))?;
    let temp = path.with_extension("json.tmp");
    tokio::fs::write(&temp, bytes)
        .await
        .map_err(|error| WebError::Internal(error.to_string()))?;
    tokio::fs::rename(temp, path)
        .await
        .map_err(|error| WebError::Internal(error.to_string()))?;
    let event =
        serde_json::to_value(&saved).map_err(|error| WebError::Internal(error.to_string()))?;
    let _ = state.event_tx.send(WebEvent::PreferencesChanged(event));
    Ok(Json(saved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn preferences_persist_partial_updates_and_broadcast() -> anyhow::Result<()> {
        let _config = crate::web::test_support::ConfigRedirect::new();
        let state = crate::web::test_support::test_server_state();
        let mut events = state.event_tx.subscribe();
        let auth = || AuthUser {
            subject: "test".into(),
        };
        let initial = serde_json::to_value(get(auth()).await?.0)?;
        assert!(initial["appearance"].is_null());
        let patch =
            serde_json::from_value(serde_json::json!({"appearance":"dark","themeMode":"flat"}))?;
        put(State(state.clone()), auth(), Json(patch)).await?;
        let patch = serde_json::from_value(serde_json::json!({"density":"compact"}))?;
        put(State(state), auth(), Json(patch)).await?;
        let saved = serde_json::to_value(get(auth()).await?.0)?;
        assert_eq!(
            saved,
            serde_json::json!({"appearance":"dark","themeMode":"flat","density":"compact"})
        );
        assert!(matches!(
            events.try_recv()?,
            WebEvent::PreferencesChanged(_)
        ));
        assert!(matches!(
            events.try_recv()?,
            WebEvent::PreferencesChanged(_)
        ));
        assert!(
            serde_json::from_value::<Preferences>(serde_json::json!({"themeMode":"unknown"}))
                .is_err()
        );
        Ok(())
    }
}
