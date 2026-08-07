//! Routine definition CRUD and run records.

use chrono::Utc;

use super::super::types::*;
use super::uuid_like_id;

impl super::WebStateHandle {
    /// List routines and recent runs.
    pub async fn list_routines(&self) -> (Vec<RoutineDefinition>, Vec<RoutineRunRecord>) {
        let state = self.inner.read().await;
        let mut routines: Vec<RoutineDefinition> = state.routines.values().cloned().collect();
        routines.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        let runs = state.routine_runs.clone();
        (routines, runs)
    }

    /// Create a routine.
    pub async fn create_routine(&self, req: CreateRoutineRequest) -> RoutineDefinition {
        let now = Utc::now().to_rfc3339();
        let routine = RoutineDefinition {
            id: format!("routine-{}", uuid_like_id()),
            name: req.name,
            trigger: req.trigger,
            enabled: req.enabled,
            cron_expr: req.cron_expr,
            timezone: req.timezone,
            target_mode: req.target_mode,
            session_id: req.session_id,
            project_index: req.project_index,
            prompt: req.prompt,
            provider_id: req.provider_id,
            model_id: req.model_id,
            last_run_at: None,
            next_run_at: None,
            last_error: None,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut state = self.inner.write().await;
        state.routines.insert(routine.id.clone(), routine.clone());
        drop(state);
        self.schedule_persist();
        self.broadcast_routine_update();
        routine
    }

    /// Update a routine.
    pub async fn update_routine(
        &self,
        routine_id: &str,
        req: UpdateRoutineRequest,
    ) -> Option<RoutineDefinition> {
        let mut state = self.inner.write().await;
        let routine = state.routines.get_mut(routine_id)?;

        if let Some(name) = req.name {
            routine.name = name;
        }
        if let Some(trigger) = req.trigger {
            routine.trigger = trigger;
        }
        if let Some(enabled) = req.enabled {
            routine.enabled = enabled;
        }
        if let Some(cron_expr) = req.cron_expr {
            routine.cron_expr = cron_expr;
        }
        if let Some(timezone) = req.timezone {
            routine.timezone = timezone;
        }
        if let Some(target_mode) = req.target_mode {
            routine.target_mode = target_mode;
        }
        if let Some(session_id) = req.session_id {
            routine.session_id = session_id;
        }
        if let Some(project_index) = req.project_index {
            routine.project_index = project_index;
        }
        if let Some(prompt) = req.prompt {
            routine.prompt = prompt;
        }
        if let Some(provider_id) = req.provider_id {
            routine.provider_id = provider_id;
        }
        if let Some(model_id) = req.model_id {
            routine.model_id = model_id;
        }
        routine.updated_at = Utc::now().to_rfc3339();
        let updated = routine.clone();
        let routine_id = updated.id.clone();
        drop(state);
        self.schedule_persist();
        // Immediately recompute next_run_at if cron changed
        self.recompute_next_run_if_scheduled(&routine_id).await;
        self.broadcast_routine_update();
        Some(updated)
    }

    /// Delete a routine.
    pub async fn delete_routine(&self, routine_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.routines.remove(routine_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
            self.broadcast_routine_update();
        }
        removed
    }

    /// Record a routine run.
    pub async fn record_routine_run(
        &self,
        routine_id: &str,
        summary: String,
        target_session_id: Option<String>,
        duration_ms: Option<u64>,
        status: &str,
    ) -> RoutineRunRecord {
        let now = Utc::now().to_rfc3339();
        let run = RoutineRunRecord {
            id: format!("routine-run-{}", uuid_like_id()),
            routine_id: routine_id.to_string(),
            status: status.to_string(),
            summary,
            target_session_id,
            duration_ms,
            created_at: now.clone(),
        };

        let mut state = self.inner.write().await;
        // Update last_run_at on the routine itself
        if let Some(routine) = state.routines.get_mut(routine_id) {
            routine.last_run_at = Some(now);
            if status == "failed" {
                routine.last_error = Some(run.summary.clone());
            } else {
                routine.last_error = None;
            }
        }
        state.routine_runs.insert(0, run.clone());
        if state.routine_runs.len() > 100 {
            state.routine_runs.truncate(100);
        }
        drop(state);
        self.schedule_persist();
        self.broadcast_routine_update();
        run
    }

    /// Broadcast a routine update event via SSE.
    fn broadcast_routine_update(&self) {
        let _ = self.event_tx.send(WebEvent::RoutineUpdated);
    }
}

#[cfg(test)]
#[path = "assistant_routines_tests.rs"]
mod assistant_routines_tests;
