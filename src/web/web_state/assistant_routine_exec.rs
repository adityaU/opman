//! Routine execution: dispatch a routine's message to a new or existing session.

use super::super::types::*;

impl super::WebStateHandle {
    /// Execute a routine: send its prompt to the target session.
    /// Returns the run record.
    pub async fn execute_routine(&self, routine_id: &str) -> Result<RoutineRunRecord, String> {
        let routine = {
            let state = self.inner.read().await;
            state
                .routines
                .get(routine_id)
                .cloned()
                .ok_or_else(|| "Routine not found".to_string())?
        };

        let prompt = routine.prompt.as_deref().unwrap_or("").trim();
        if prompt.is_empty() {
            let _run = self
                .record_routine_run(
                    routine_id,
                    "No prompt configured".to_string(),
                    None,
                    None,
                    "failed",
                )
                .await;
            return Err(format!(
                "Routine '{}' has no prompt configured",
                routine.name
            ));
        }

        let start = std::time::Instant::now();

        // Determine session ID
        let session_id = match routine.target_mode.as_ref() {
            Some(RoutineTargetMode::NewSession) => {
                // Create a new session for this routine
                let project_index = routine.project_index.unwrap_or(0);
                match self.create_session_for_routine(project_index).await {
                    Ok(id) => id,
                    Err(e) => {
                        let _run = self
                            .record_routine_run(
                                routine_id,
                                format!("Failed to create session: {e}"),
                                None,
                                None,
                                "failed",
                            )
                            .await;
                        return Err(e);
                    }
                }
            }
            _ => {
                // Use existing session
                match routine.session_id.as_deref() {
                    Some(id) if !id.is_empty() => id.to_string(),
                    _ => {
                        let _run = self
                            .record_routine_run(
                                routine_id,
                                "No target session configured".to_string(),
                                None,
                                None,
                                "failed",
                            )
                            .await;
                        return Err("No target session configured".to_string());
                    }
                }
            }
        };

        let project_index = routine.project_index.unwrap_or(0);

        // Build optional model override from routine config
        let model_ref = match (routine.provider_id.as_deref(), routine.model_id.as_deref()) {
            (Some(pid), Some(mid)) if !pid.is_empty() && !mid.is_empty() => {
                Some(crate::web::types::ModelRef {
                    provider_id: pid.to_string(),
                    model_id: mid.to_string(),
                })
            }
            _ => None,
        };

        // Send the message
        if let Err(e) = self
            .send_to_session(&session_id, &project_index, prompt, model_ref.as_ref())
            .await
        {
            let elapsed = start.elapsed().as_millis() as u64;
            let _run = self
                .record_routine_run(
                    routine_id,
                    format!("Failed to send message: {e}"),
                    Some(session_id),
                    Some(elapsed),
                    "failed",
                )
                .await;
            return Err(e);
        }

        let elapsed = start.elapsed().as_millis() as u64;

        let run = self
            .record_routine_run(
                routine_id,
                format!(
                    "Sent message to session {}",
                    &session_id[..session_id.len().min(12)]
                ),
                Some(session_id),
                Some(elapsed),
                "completed",
            )
            .await;

        Ok(run)
    }

    /// Fire any enabled `OnSessionIdle` routines bound to the given session.
    ///
    /// Called from the SSE handler when a session transitions to "idle".
    /// A 60-second cooldown per routine prevents infinite self-loops
    /// (routine sends message → session busy → session idle → routine fires again).
    pub(super) async fn try_fire_idle_routines(&self, session_id: &str) {
        let now = std::time::Instant::now();
        let cooldown = std::time::Duration::from_secs(60);

        let due_ids: Vec<String> = {
            let state = self.inner.read().await;
            state
                .routines
                .values()
                .filter(|r| {
                    r.enabled
                        && r.trigger == super::super::types::RoutineTrigger::OnSessionIdle
                        && r.session_id.as_deref() == Some(session_id)
                })
                .filter(|r| {
                    // Skip if this routine fired within the cooldown window
                    state
                        .routine_idle_cooldown
                        .get(&r.id)
                        .map_or(true, |last| now.duration_since(*last) >= cooldown)
                })
                .map(|r| r.id.clone())
                .collect()
        };

        for id in due_ids {
            // Record the fire time *before* executing so that even if execution
            // is slow, subsequent idle transitions are suppressed.
            {
                let mut state = self.inner.write().await;
                state.routine_idle_cooldown.insert(id.clone(), now);
            }
            tracing::debug!(routine_id = %id, session_id = %session_id, "firing on_session_idle routine");
            if let Err(e) = self.execute_routine(&id).await {
                tracing::warn!(routine_id = %id, error = %e, "on_session_idle routine failed");
            }
        }
    }

    /// Create a new session for a routine, returning the session ID.
    async fn create_session_for_routine(&self, project_index: usize) -> Result<String, String> {
        let dir = {
            let state = self.inner.read().await;
            state
                .projects
                .get(project_index)
                .map(|p| p.path.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        if dir.is_empty() {
            return Err("No project directory found".to_string());
        }

        let base = crate::app::base_url().to_string();
        let url = format!("{}/session", base);

        let client = reqwest::Client::new();
        match client
            .post(&url)
            .header("x-opencode-directory", &dir)
            .header("Accept", "application/json")
            .json(&serde_json::json!({}))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let body: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Failed to parse session response: {e}"))?;
                super::assistant_send::parse_session_id_from_body(&body)
            }
            Ok(resp) => Err(format!("Failed to create session: HTTP {}", resp.status())),
            Err(e) => Err(format!("Failed to create session: {e}")),
        }
    }
}

#[cfg(test)]
#[path = "assistant_routine_upstream_tests.rs"]
mod assistant_routine_upstream_tests;

#[cfg(test)]
#[path = "assistant_routine_exec_tests.rs"]
mod assistant_routine_exec_tests;
