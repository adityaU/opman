use chrono::Utc;

use super::super::types::*;
use super::uuid_like_id;

impl super::WebStateHandle {
    // ── Missions ────────────────────────────────────────────────────

    /// List all saved missions.
    pub async fn list_missions(&self) -> Vec<Mission> {
        let state = self.inner.read().await;
        let mut list: Vec<Mission> = state.missions.values().cloned().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Create a new mission.
    pub async fn create_mission(&self, req: CreateMissionRequest) -> Mission {
        let now = Utc::now().to_rfc3339();
        let mission = Mission {
            id: format!("mission-{}", uuid_like_id()),
            title: req.title,
            goal: req.goal,
            next_action: req.next_action,
            status: req.status.unwrap_or(MissionStatus::Planned),
            project_index: req.project_index,
            session_id: req.session_id,
            created_at: now.clone(),
            updated_at: now,
        };

        let mut state = self.inner.write().await;
        state.missions.insert(mission.id.clone(), mission.clone());
        drop(state);
        self.schedule_persist();
        mission
    }

    /// Update an existing mission.
    pub async fn update_mission(&self, mission_id: &str, req: UpdateMissionRequest) -> Option<Mission> {
        let mut state = self.inner.write().await;
        let mission = state.missions.get_mut(mission_id)?;

        if let Some(title) = req.title {
            mission.title = title;
        }
        if let Some(goal) = req.goal {
            mission.goal = goal;
        }
        if let Some(next_action) = req.next_action {
            mission.next_action = next_action;
        }
        if let Some(status) = req.status {
            mission.status = status;
        }
        if let Some(project_index) = req.project_index {
            mission.project_index = project_index;
        }
        if let Some(session_id) = req.session_id {
            mission.session_id = session_id;
        }
        mission.updated_at = Utc::now().to_rfc3339();
        let updated = mission.clone();
        drop(state);
        self.schedule_persist();
        Some(updated)
    }

    /// Delete a mission by ID. Returns true if it existed.
    pub async fn delete_mission(&self, mission_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.missions.remove(mission_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }

    // ── Personal Memory ─────────────────────────────────────────────

    /// List all personal memory items.
    pub async fn list_personal_memory(&self) -> Vec<PersonalMemoryItem> {
        let state = self.inner.read().await;
        let mut list: Vec<PersonalMemoryItem> = state.personal_memory.values().cloned().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Create a personal memory item.
    pub async fn create_personal_memory(&self, req: CreatePersonalMemoryRequest) -> PersonalMemoryItem {
        let now = Utc::now().to_rfc3339();
        let item = PersonalMemoryItem {
            id: format!("memory-{}", uuid_like_id()),
            label: req.label,
            content: req.content,
            scope: req.scope,
            project_index: req.project_index,
            session_id: req.session_id,
            created_at: now.clone(),
            updated_at: now,
        };

        let mut state = self.inner.write().await;
        state.personal_memory.insert(item.id.clone(), item.clone());
        drop(state);
        self.schedule_persist();
        item
    }

    /// Update a personal memory item.
    pub async fn update_personal_memory(
        &self,
        memory_id: &str,
        req: UpdatePersonalMemoryRequest,
    ) -> Option<PersonalMemoryItem> {
        let mut state = self.inner.write().await;
        let item = state.personal_memory.get_mut(memory_id)?;

        if let Some(label) = req.label {
            item.label = label;
        }
        if let Some(content) = req.content {
            item.content = content;
        }
        if let Some(scope) = req.scope {
            item.scope = scope;
        }
        if let Some(project_index) = req.project_index {
            item.project_index = project_index;
        }
        if let Some(session_id) = req.session_id {
            item.session_id = session_id;
        }
        item.updated_at = Utc::now().to_rfc3339();
        let updated = item.clone();
        drop(state);
        self.schedule_persist();
        Some(updated)
    }

    /// Delete a personal memory item.
    pub async fn delete_personal_memory(&self, memory_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.personal_memory.remove(memory_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }

    // ── Autonomy Controls ───────────────────────────────────────────

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

    // ── Routines ────────────────────────────────────────────────────

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
            action: req.action,
            mission_id: req.mission_id,
            session_id: req.session_id,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut state = self.inner.write().await;
        state.routines.insert(routine.id.clone(), routine.clone());
        drop(state);
        self.schedule_persist();
        routine
    }

    /// Update a routine.
    pub async fn update_routine(&self, routine_id: &str, req: UpdateRoutineRequest) -> Option<RoutineDefinition> {
        let mut state = self.inner.write().await;
        let routine = state.routines.get_mut(routine_id)?;

        if let Some(name) = req.name {
            routine.name = name;
        }
        if let Some(trigger) = req.trigger {
            routine.trigger = trigger;
        }
        if let Some(action) = req.action {
            routine.action = action;
        }
        if let Some(mission_id) = req.mission_id {
            routine.mission_id = mission_id;
        }
        if let Some(session_id) = req.session_id {
            routine.session_id = session_id;
        }
        routine.updated_at = Utc::now().to_rfc3339();
        let updated = routine.clone();
        drop(state);
        self.schedule_persist();
        Some(updated)
    }

    /// Delete a routine.
    pub async fn delete_routine(&self, routine_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.routines.remove(routine_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }

    /// Record a routine run.
    pub async fn record_routine_run(&self, routine_id: &str, summary: String) -> RoutineRunRecord {
        let run = RoutineRunRecord {
            id: format!("routine-run-{}", uuid_like_id()),
            routine_id: routine_id.to_string(),
            status: "completed".to_string(),
            summary,
            created_at: Utc::now().to_rfc3339(),
        };
        let mut state = self.inner.write().await;
        state.routine_runs.insert(0, run.clone());
        if state.routine_runs.len() > 50 {
            state.routine_runs.truncate(50);
        }
        drop(state);
        self.schedule_persist();
        run
    }

}
