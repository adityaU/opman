//! Backend intelligence: stats, signals, templates, filtered memory.

use chrono::Utc;

use super::super::types::*;
use super::uuid_like_id;

impl super::WebStateHandle {
    // ── Assistant Center Stats ───────────────────────────────────────

    /// Compute dashboard statistics for the assistant center.
    pub async fn build_assistant_stats(
        &self,
        req: AssistantCenterStatsRequest,
    ) -> AssistantCenterStats {
        let missions = self.list_missions().await;
        let memory = self.list_personal_memory().await;
        let (routines, _) = self.list_routines().await;
        let delegated = self.list_delegated_work().await;
        let workspaces = self.list_workspaces().await;
        let autonomy = self.get_autonomy_settings().await;

        let active_missions = missions
            .iter()
            .filter(|m| matches!(m.status, MissionStatus::Active))
            .count();
        let blocked_missions = missions
            .iter()
            .filter(|m| matches!(m.status, MissionStatus::Blocked))
            .count();
        let active_delegations = delegated
            .iter()
            .filter(|d| !matches!(d.status, DelegationStatus::Completed))
            .count();

        let mode_str = match autonomy.mode {
            AutonomyMode::Observe => "observe",
            AutonomyMode::Nudge => "nudge",
            AutonomyMode::Continue => "continue",
            AutonomyMode::Autonomous => "autonomous",
        };

        AssistantCenterStats {
            active_missions,
            blocked_missions,
            total_missions: missions.len(),
            pending_permissions: req.permissions.len(),
            pending_questions: req.questions.len(),
            memory_items: memory.len(),
            active_routines: routines.len(),
            active_delegations,
            workspace_count: workspaces.len(),
            autonomy_mode: mode_str.to_string(),
        }
    }

    // ── Signals ─────────────────────────────────────────────────────

    /// List all stored signals (newest first).
    pub async fn list_signals(&self) -> Vec<SignalInput> {
        let state = self.inner.read().await;
        state.signals.clone()
    }

    /// Add a new signal to the persistent store.
    pub async fn add_signal(&self, req: AddSignalRequest) -> SignalInput {
        let signal = SignalInput {
            id: format!("signal-{}", uuid_like_id()),
            kind: req.kind,
            title: req.title,
            body: req.body,
            created_at: Utc::now().timestamp_millis() as f64,
            session_id: req.session_id,
        };
        let mut state = self.inner.write().await;
        state.signals.insert(0, signal.clone());
        if state.signals.len() > 100 {
            state.signals.truncate(100);
        }
        drop(state);
        self.schedule_persist();
        signal
    }

    // ── Workspace Templates ─────────────────────────────────────────

    /// Return built-in workspace templates.
    pub fn workspace_templates() -> Vec<WorkspaceTemplate> {
        vec![
            WorkspaceTemplate {
                id: "tpl-focus".to_string(),
                name: "Focus Mode".to_string(),
                description: "Chat only — minimal distractions".to_string(),
                panels: WorkspacePanels {
                    sidebar: false,
                    terminal: false,
                    editor: false,
                    git: false,
                },
                layout: WorkspaceLayout::default(),
            },
            WorkspaceTemplate {
                id: "tpl-dev".to_string(),
                name: "Development".to_string(),
                description: "Chat + editor + terminal for active coding"
                    .to_string(),
                panels: WorkspacePanels {
                    sidebar: true,
                    terminal: true,
                    editor: true,
                    git: false,
                },
                layout: WorkspaceLayout::default(),
            },
            WorkspaceTemplate {
                id: "tpl-review".to_string(),
                name: "Code Review".to_string(),
                description: "Chat + editor + git panel for reviewing changes"
                    .to_string(),
                panels: WorkspacePanels {
                    sidebar: true,
                    terminal: false,
                    editor: true,
                    git: true,
                },
                layout: WorkspaceLayout::default(),
            },
            WorkspaceTemplate {
                id: "tpl-morning".to_string(),
                name: "Morning Review".to_string(),
                description: "Full workspace for daily standup and planning"
                    .to_string(),
                panels: WorkspacePanels {
                    sidebar: true,
                    terminal: true,
                    editor: true,
                    git: true,
                },
                layout: WorkspaceLayout::default(),
            },
        ]
    }

    // ── Filtered Memory ─────────────────────────────────────────────

    /// Return memory items filtered to the active scope.
    ///
    /// Active scope means: global items, project-scoped items matching the
    /// given `project_index`, and session-scoped items matching the given
    /// `session_id`.
    pub async fn list_active_memory(
        &self,
        project_index: Option<usize>,
        session_id: Option<&str>,
    ) -> Vec<PersonalMemoryItem> {
        let all = self.list_personal_memory().await;
        all.into_iter()
            .filter(|m| {
                match m.scope {
                    MemoryScope::Global => true,
                    MemoryScope::Project => {
                        project_index.is_some() && m.project_index == project_index
                    }
                    MemoryScope::Session => {
                        session_id.is_some()
                            && m.session_id.as_deref() == session_id
                    }
                }
            })
            .collect()
    }
}
