use chrono::Utc;

use super::super::types::*;
use super::uuid_like_id;

impl super::WebStateHandle {
    // ── Missions (v2: goal-driven loop) ─────────────────────────────

    /// List all saved missions.
    pub async fn list_missions(&self) -> Vec<Mission> {
        let state = self.inner.read().await;
        let mut list: Vec<Mission> = state.missions.values().cloned().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Get a single mission by ID.
    pub async fn get_mission(&self, mission_id: &str) -> Option<Mission> {
        let state = self.inner.read().await;
        state.missions.get(mission_id).cloned()
    }

    /// Create a new mission (state = Pending).
    pub async fn create_mission(&self, req: CreateMissionRequest) -> Mission {
        let now = Utc::now().to_rfc3339();

        // Determine project index
        let project_index = {
            let state = self.inner.read().await;
            req.project_index.unwrap_or(state.active_project)
        };

        // Determine session ID — use provided or empty (will be set on start)
        let session_id = req.session_id.unwrap_or_default();

        let mission = Mission {
            id: format!("mission-{}", uuid_like_id()),
            goal: req.goal,
            session_id,
            project_index,
            state: MissionState::Pending,
            iteration: 0,
            max_iterations: req.max_iterations.unwrap_or(10),
            last_verdict: None,
            last_eval_summary: None,
            eval_history: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        };

        let mut state = self.inner.write().await;
        state.missions.insert(mission.id.clone(), mission.clone());
        drop(state);
        self.schedule_persist();
        self.broadcast_mission_update(&mission);
        mission
    }

    /// Update mission fields (goal, max_iterations).
    pub async fn update_mission(
        &self,
        mission_id: &str,
        req: UpdateMissionRequest,
    ) -> Option<Mission> {
        let mut state = self.inner.write().await;
        let mission = state.missions.get_mut(mission_id)?;

        if let Some(goal) = req.goal {
            mission.goal = goal;
        }
        if let Some(max_iterations) = req.max_iterations {
            mission.max_iterations = max_iterations;
        }
        mission.updated_at = Utc::now().to_rfc3339();
        let updated = mission.clone();
        drop(state);
        self.schedule_persist();
        self.broadcast_mission_update(&updated);
        Some(updated)
    }

    /// Delete a mission by ID.
    pub async fn delete_mission(&self, mission_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.missions.remove(mission_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
            let _ = self.event_tx.send(WebEvent::StateChanged);
        }
        removed
    }

    /// Perform a lifecycle action on a mission.
    pub async fn mission_action(
        &self,
        mission_id: &str,
        action: MissionAction,
    ) -> Result<Mission, String> {
        let mut state = self.inner.write().await;
        let mission = state
            .missions
            .get_mut(mission_id)
            .ok_or_else(|| "Mission not found".to_string())?;

        match action {
            MissionAction::Start => {
                if mission.state != MissionState::Pending {
                    return Err(format!(
                        "Cannot start mission in state {:?}",
                        mission.state
                    ));
                }
                mission.state = MissionState::Executing;
                mission.iteration = 1;
                mission.updated_at = Utc::now().to_rfc3339();
            }
            MissionAction::Pause => {
                if !matches!(
                    mission.state,
                    MissionState::Executing | MissionState::Evaluating
                ) {
                    return Err(format!(
                        "Cannot pause mission in state {:?}",
                        mission.state
                    ));
                }
                mission.state = MissionState::Paused;
                mission.updated_at = Utc::now().to_rfc3339();
            }
            MissionAction::Resume => {
                if mission.state != MissionState::Paused {
                    return Err(format!(
                        "Cannot resume mission in state {:?}",
                        mission.state
                    ));
                }
                mission.state = MissionState::Executing;
                mission.updated_at = Utc::now().to_rfc3339();
            }
            MissionAction::Cancel => {
                if matches!(
                    mission.state,
                    MissionState::Completed | MissionState::Cancelled | MissionState::Failed
                ) {
                    return Err(format!(
                        "Cannot cancel mission in state {:?}",
                        mission.state
                    ));
                }
                mission.state = MissionState::Cancelled;
                mission.updated_at = Utc::now().to_rfc3339();
            }
        }

        let updated = mission.clone();
        let should_start_execution = matches!(action, MissionAction::Start | MissionAction::Resume)
            && updated.state == MissionState::Executing;
        drop(state);
        self.schedule_persist();
        self.broadcast_mission_update(&updated);

        // Kick off execution if we just started/resumed
        if should_start_execution {
            self.kick_mission_execution(&updated).await;
        }

        Ok(updated)
    }

    // ── Mission loop engine ─────────────────────────────────────────

    /// Called when a mission session goes idle. This is the core loop:
    /// if the mission is in Executing state, transition to Evaluating
    /// and send an evaluator prompt into the same session.
    pub(super) async fn on_mission_session_idle(&self, session_id: &str) {
        // Find an active mission for this session
        let mission = {
            let state = self.inner.read().await;
            state.missions.values().find(|m| {
                m.session_id == session_id
                    && matches!(m.state, MissionState::Executing)
            }).cloned()
        };

        let Some(mission) = mission else { return };

        // Transition to Evaluating
        {
            let mut state = self.inner.write().await;
            if let Some(m) = state.missions.get_mut(&mission.id) {
                m.state = MissionState::Evaluating;
                m.updated_at = Utc::now().to_rfc3339();
                self.broadcast_mission_update(m);
            }
        }
        self.schedule_persist();

        // Send evaluator prompt
        self.send_evaluator_prompt(&mission).await;
    }

    /// Called when a mission session goes idle while in Evaluating state.
    /// Parse the evaluator's response and decide next step.
    pub(super) async fn on_mission_evaluation_complete(&self, session_id: &str) {
        let mission = {
            let state = self.inner.read().await;
            state.missions.values().find(|m| {
                m.session_id == session_id
                    && matches!(m.state, MissionState::Evaluating)
            }).cloned()
        };

        let Some(mission) = mission else { return };

        // Fetch the latest assistant message to parse the evaluation
        let eval_result = self.parse_latest_eval_response(&mission).await;

        let now = Utc::now().to_rfc3339();
        let record = EvalRecord {
            iteration: mission.iteration,
            verdict: eval_result.verdict.clone(),
            summary: eval_result.summary.clone(),
            next_step: eval_result.next_step.clone(),
            timestamp: now.clone(),
        };

        // Update mission state based on verdict
        let (new_state, new_iteration) = match eval_result.verdict {
            EvalVerdict::Achieved => (MissionState::Completed, mission.iteration),
            EvalVerdict::Failed => (MissionState::Failed, mission.iteration),
            EvalVerdict::Blocked => (MissionState::Paused, mission.iteration),
            EvalVerdict::Continue => {
                let next_iter = mission.iteration + 1;
                if mission.max_iterations > 0 && next_iter > mission.max_iterations {
                    (MissionState::Failed, mission.iteration)
                } else {
                    (MissionState::Executing, next_iter)
                }
            }
        };

        let should_continue = new_state == MissionState::Executing;

        let updated = {
            let mut state = self.inner.write().await;
            if let Some(m) = state.missions.get_mut(&mission.id) {
                m.state = new_state;
                m.iteration = new_iteration;
                m.last_verdict = Some(eval_result.verdict);
                m.last_eval_summary = Some(eval_result.summary);
                m.eval_history.push(record);
                m.updated_at = now;
                m.clone()
            } else {
                return;
            }
        };
        self.schedule_persist();
        self.broadcast_mission_update(&updated);

        // If continuing, send the next execution prompt
        if should_continue {
            self.send_continuation_prompt(&updated, eval_result.next_step.as_deref()).await;
        }
    }

    /// Kick off the initial execution of a mission.
    async fn kick_mission_execution(&self, mission: &Mission) {
        if mission.session_id.is_empty() {
            tracing::warn!(
                mission_id = %mission.id,
                "Cannot execute mission without a session_id"
            );
            return;
        }

        let prompt = format!(
            "You are working on a mission. Your goal is:\n\n\
             {}\n\n\
             This is iteration {} of the mission. \
             Work toward achieving this goal. When you believe you have made \
             meaningful progress or completed the goal, stop and let me know \
             what you accomplished.",
            mission.goal,
            mission.iteration,
        );

        let _ = self.send_to_session(&mission.session_id, &mission.project_index, &prompt, None).await;
    }

    /// Send the evaluator prompt into the session.
    async fn send_evaluator_prompt(&self, mission: &Mission) {
        let history_context = if mission.eval_history.is_empty() {
            String::new()
        } else {
            let entries: Vec<String> = mission.eval_history.iter().map(|r| {
                format!(
                    "- Iteration {}: {:?} — {}",
                    r.iteration,
                    r.verdict,
                    r.summary,
                )
            }).collect();
            format!("\n\nPrevious evaluation history:\n{}", entries.join("\n"))
        };

        let prompt = format!(
            "EVALUATOR MODE — Assess whether the following goal has been achieved \
             based on the work done in this session so far.\n\n\
             Goal: {}\n\
             Current iteration: {}/{}{}\n\n\
             Respond with a JSON object (and nothing else before or after it) in this exact format:\n\
             {{\n  \
               \"verdict\": \"achieved\" | \"continue\" | \"blocked\" | \"failed\",\n  \
               \"summary\": \"Brief assessment of current state\",\n  \
               \"next_step\": \"What to do next (if continuing)\"\n\
             }}\n\n\
             - Use \"achieved\" if the goal has been fully met.\n\
             - Use \"continue\" if progress was made but the goal is not yet complete.\n\
             - Use \"blocked\" if you cannot proceed without user input.\n\
             - Use \"failed\" if the goal is not achievable.",
            mission.goal,
            mission.iteration,
            if mission.max_iterations == 0 {
                "∞".to_string()
            } else {
                mission.max_iterations.to_string()
            },
            history_context,
        );

        let _ = self.send_to_session(&mission.session_id, &mission.project_index, &prompt, None).await;
    }

    /// Send a continuation prompt after evaluation says "continue".
    async fn send_continuation_prompt(&self, mission: &Mission, next_step: Option<&str>) {
        let step_instruction = next_step
            .map(|s| format!("The evaluator suggests the next step is: {}\n\n", s))
            .unwrap_or_default();

        let prompt = format!(
            "Continue working on the mission goal:\n\n\
             {}\n\n\
             {}\
             This is iteration {} of the mission. \
             Continue making progress toward the goal.",
            mission.goal,
            step_instruction,
            mission.iteration,
        );

        let _ = self.send_to_session(&mission.session_id, &mission.project_index, &prompt, None).await;
    }

    /// Send a message to a session via the opencode proxy.
    ///
    /// An optional `ModelRef` can be provided to override the model for this message.
    /// Returns `Ok(())` on success, or `Err(description)` on failure.
    async fn send_to_session(
        &self,
        session_id: &str,
        project_index: &usize,
        message: &str,
        model: Option<&crate::web::types::ModelRef>,
    ) -> Result<(), String> {
        let dir = {
            let state = self.inner.read().await;
            state.projects.get(*project_index)
                .map(|p| p.path.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        if dir.is_empty() {
            tracing::warn!(
                session_id = %session_id,
                "Cannot send message: no project directory found"
            );
            return Err("No project directory found".to_string());
        }

        let base = crate::app::base_url().to_string();
        let url = format!("{}/session/{}/message", base, session_id);

        let mut body = serde_json::json!({
            "parts": [{ "type": "text", "text": message }]
        });
        if let Some(model_ref) = model {
            body["model"] = serde_json::json!({
                "providerID": model_ref.provider_id,
                "modelID": model_ref.model_id,
            });
        }

        let client = reqwest::Client::new();
        match client
            .post(&url)
            .header("x-opencode-directory", &dir)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!(
                    session_id = %session_id,
                    "Message sent successfully"
                );
                Ok(())
            }
            Ok(resp) => {
                let status = resp.status();
                let detail = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    session_id = %session_id,
                    status = %status,
                    detail = %detail,
                    "Message rejected by upstream"
                );
                Err(format!("Upstream rejected message: HTTP {status}"))
            }
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to send message"
                );
                Err(format!("Failed to send message: {e}"))
            }
        }
    }

    /// Parse the latest assistant message from a session as an evaluation response.
    async fn parse_latest_eval_response(&self, mission: &Mission) -> EvalResult {
        let dir = {
            let state = self.inner.read().await;
            state.projects.get(mission.project_index)
                .map(|p| p.path.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        if dir.is_empty() {
            return EvalResult::default_continue("Could not read session");
        }

        let base = crate::app::base_url().to_string();
        let url = format!("{}/session/{}/message", base, mission.session_id);

        let client = reqwest::Client::new();
        let resp = match client
            .get(&url)
            .header("x-opencode-directory", &dir)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return EvalResult::default_continue(&format!("Fetch error: {e}"));
            }
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                return EvalResult::default_continue(&format!("Parse error: {e}"));
            }
        };

        // Messages can be an array or object keyed by ID
        let messages: Vec<serde_json::Value> = if let Some(arr) = body.as_array() {
            arr.clone()
        } else if let Some(obj) = body.as_object() {
            obj.values().cloned().collect()
        } else {
            return EvalResult::default_continue("No messages found");
        };

        // Find the latest assistant message by time
        let latest_assistant = messages
            .iter()
            .filter(|m| {
                m.pointer("/info/role").and_then(|v| v.as_str()) == Some("assistant")
            })
            .max_by_key(|m| {
                m.pointer("/info/time/created").and_then(|v| v.as_u64()).unwrap_or(0)
            });

        let Some(msg) = latest_assistant else {
            return EvalResult::default_continue("No assistant response found");
        };

        // Extract text content from the message parts
        let text = extract_message_text(msg);
        if text.is_empty() {
            return EvalResult::default_continue("Empty assistant response");
        }

        // Try to parse JSON from the response
        parse_eval_json(&text)
    }

    /// Broadcast a mission update event via SSE.
    fn broadcast_mission_update(&self, mission: &Mission) {
        let payload = serde_json::to_value(mission).unwrap_or_default();
        let _ = self.event_tx.send(WebEvent::MissionUpdated { mission: payload });
    }

    /// Called from the SSE handler when a session becomes idle.
    /// Routes to the correct mission loop step based on current state.
    pub(super) async fn try_advance_mission(&self, session_id: &str) {
        // Check if any mission is bound to this session and in an active state
        let mission_state = {
            let state = self.inner.read().await;
            state.missions.values()
                .find(|m| m.session_id == session_id
                    && matches!(m.state, MissionState::Executing | MissionState::Evaluating))
                .map(|m| m.state.clone())
        };

        match mission_state {
            Some(MissionState::Executing) => {
                self.on_mission_session_idle(session_id).await;
            }
            Some(MissionState::Evaluating) => {
                self.on_mission_evaluation_complete(session_id).await;
            }
            _ => {} // No active mission for this session
        }
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
            enabled: req.enabled,
            cron_expr: req.cron_expr,
            timezone: req.timezone,
            target_mode: req.target_mode,
            session_id: req.session_id,
            project_index: req.project_index,
            prompt: req.prompt,
            provider_id: req.provider_id,
            model_id: req.model_id,
            mission_id: req.mission_id,
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
        if let Some(mission_id) = req.mission_id {
            routine.mission_id = mission_id;
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

    /// Execute a routine: send its prompt to the target session.
    /// Returns the run record.
    pub async fn execute_routine(&self, routine_id: &str) -> Result<RoutineRunRecord, String> {
        let routine = {
            let state = self.inner.read().await;
            state.routines.get(routine_id).cloned()
                .ok_or_else(|| "Routine not found".to_string())?
        };

        if routine.action != RoutineAction::SendMessage {
            // Legacy actions don't have backend execution — just record the run
            let run = self.record_routine_run(
                routine_id,
                format!("Executed legacy action: {:?}", routine.action),
                None, None, "completed",
            ).await;
            return Ok(run);
        }

        let prompt = routine.prompt.as_deref().unwrap_or("").trim();
        if prompt.is_empty() {
            let _run = self.record_routine_run(
                routine_id, "No prompt configured".to_string(),
                None, None, "failed",
            ).await;
            return Err(format!("Routine '{}' has no prompt configured", routine.name));
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
                        let _run = self.record_routine_run(
                            routine_id, format!("Failed to create session: {e}"),
                            None, None, "failed",
                        ).await;
                        return Err(e);
                    }
                }
            }
            _ => {
                // Use existing session
                match routine.session_id.as_deref() {
                    Some(id) if !id.is_empty() => id.to_string(),
                    _ => {
                        let _run = self.record_routine_run(
                            routine_id, "No target session configured".to_string(),
                            None, None, "failed",
                        ).await;
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
        if let Err(e) = self.send_to_session(&session_id, &project_index, prompt, model_ref.as_ref()).await {
            let elapsed = start.elapsed().as_millis() as u64;
            let _run = self.record_routine_run(
                routine_id,
                format!("Failed to send message: {e}"),
                Some(session_id),
                Some(elapsed),
                "failed",
            ).await;
            return Err(e);
        }

        let elapsed = start.elapsed().as_millis() as u64;

        let run = self.record_routine_run(
            routine_id,
            format!("Sent message to session {}", &session_id[..session_id.len().min(12)]),
            Some(session_id),
            Some(elapsed),
            "completed",
        ).await;

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
                        && r.action == super::super::types::RoutineAction::SendMessage
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
            state.projects.get(project_index)
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
                let body: serde_json::Value = resp.json().await
                    .map_err(|e| format!("Failed to parse session response: {e}"))?;
                body.get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| "No session ID in response".to_string())
            }
            Ok(resp) => Err(format!("Failed to create session: HTTP {}", resp.status())),
            Err(e) => Err(format!("Failed to create session: {e}")),
        }
    }

    /// Broadcast a routine update event via SSE.
    fn broadcast_routine_update(&self) {
        let _ = self.event_tx.send(WebEvent::RoutineUpdated);
    }

}

// ── Eval parsing helpers ────────────────────────────────────────────

/// Parsed evaluation result.
struct EvalResult {
    verdict: EvalVerdict,
    summary: String,
    next_step: Option<String>,
}

impl EvalResult {
    fn default_continue(reason: &str) -> Self {
        Self {
            verdict: EvalVerdict::Continue,
            summary: reason.to_string(),
            next_step: None,
        }
    }
}

/// Extract text content from a message Value.
fn extract_message_text(msg: &serde_json::Value) -> String {
    // Try parts array first
    if let Some(parts) = msg.pointer("/info/parts").and_then(|v| v.as_array()) {
        let texts: Vec<&str> = parts
            .iter()
            .filter_map(|p| {
                if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                    p.get("text").and_then(|t| t.as_str())
                } else {
                    None
                }
            })
            .collect();
        if !texts.is_empty() {
            return texts.join("\n");
        }
    }

    // Fallback: try content array
    if let Some(content) = msg.pointer("/info/content").and_then(|v| v.as_array()) {
        let texts: Vec<&str> = content
            .iter()
            .filter_map(|c| {
                if c.get("type").and_then(|t| t.as_str()) == Some("text") {
                    c.get("text").and_then(|t| t.as_str())
                } else {
                    None
                }
            })
            .collect();
        return texts.join("\n");
    }

    String::new()
}

/// Try to parse evaluation JSON from assistant text.
fn parse_eval_json(text: &str) -> EvalResult {
    // Try to find JSON object in the text
    // Look for { ... } pattern
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };

    #[derive(serde::Deserialize)]
    struct EvalJson {
        verdict: String,
        #[serde(default)]
        summary: String,
        #[serde(default)]
        next_step: Option<String>,
    }

    match serde_json::from_str::<EvalJson>(json_str) {
        Ok(parsed) => {
            let verdict = match parsed.verdict.as_str() {
                "achieved" => EvalVerdict::Achieved,
                "failed" => EvalVerdict::Failed,
                "blocked" => EvalVerdict::Blocked,
                _ => EvalVerdict::Continue,
            };
            EvalResult {
                verdict,
                summary: if parsed.summary.is_empty() {
                    "Evaluation complete".to_string()
                } else {
                    parsed.summary
                },
                next_step: parsed.next_step,
            }
        }
        Err(_) => {
            // Could not parse JSON — try heuristic detection
            let lower = text.to_lowercase();
            if lower.contains("achieved") || lower.contains("goal has been met") || lower.contains("completed successfully") {
                EvalResult {
                    verdict: EvalVerdict::Achieved,
                    summary: text.chars().take(200).collect(),
                    next_step: None,
                }
            } else if lower.contains("blocked") || lower.contains("need user input") {
                EvalResult {
                    verdict: EvalVerdict::Blocked,
                    summary: text.chars().take(200).collect(),
                    next_step: None,
                }
            } else if lower.contains("failed") || lower.contains("not achievable") {
                EvalResult {
                    verdict: EvalVerdict::Failed,
                    summary: text.chars().take(200).collect(),
                    next_step: None,
                }
            } else {
                // Default to continue
                EvalResult {
                    verdict: EvalVerdict::Continue,
                    summary: text.chars().take(200).collect(),
                    next_step: None,
                }
            }
        }
    }
}
