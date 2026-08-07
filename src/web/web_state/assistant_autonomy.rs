//! Autonomy mode settings on the shared web state.

use chrono::Utc;

use super::super::types::*;

impl super::WebStateHandle {
    /// Get current autonomy settings.
    pub async fn get_autonomy_settings(&self) -> AutonomySettings {
        let state = self.inner.read().await;
        state.autonomy_settings.clone()
    }

    /// Update autonomy settings.
    pub async fn update_autonomy_settings(&self, mode: AutonomyMode) -> AutonomySettings {
        let mut state = self.inner.write().await;
        state.autonomy_settings = AutonomySettings {
            mode,
            updated_at: Utc::now().to_rfc3339(),
        };
        let settings = state.autonomy_settings.clone();
        drop(state);
        self.schedule_persist();
        settings
    }
}

#[cfg(test)]
#[path = "assistant_autonomy_tests.rs"]
mod assistant_autonomy_tests;
