use chrono::Utc;

use super::super::types::*;
use super::uuid_like_id;

impl super::WebStateHandle {
    // ── Delegated Work ──────────────────────────────────────────────

    /// List delegated work items.
    pub async fn list_delegated_work(&self) -> Vec<DelegatedWorkItem> {
        let state = self.inner.read().await;
        let mut list: Vec<DelegatedWorkItem> = state.delegated_work.values().cloned().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Create delegated work item.
    pub async fn create_delegated_work(&self, req: CreateDelegatedWorkRequest) -> DelegatedWorkItem {
        let now = Utc::now().to_rfc3339();
        let item = DelegatedWorkItem {
            id: format!("delegation-{}", uuid_like_id()),
            title: req.title,
            assignee: req.assignee,
            scope: req.scope,
            status: DelegationStatus::Planned,
            mission_id: req.mission_id,
            session_id: req.session_id,
            subagent_session_id: req.subagent_session_id,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut state = self.inner.write().await;
        state.delegated_work.insert(item.id.clone(), item.clone());
        drop(state);
        self.schedule_persist();
        item
    }

    /// Update delegated work item.
    pub async fn update_delegated_work(&self, item_id: &str, req: UpdateDelegatedWorkRequest) -> Option<DelegatedWorkItem> {
        let mut state = self.inner.write().await;
        let item = state.delegated_work.get_mut(item_id)?;
        if let Some(status) = req.status {
            item.status = status;
        }
        item.updated_at = Utc::now().to_rfc3339();
        let updated = item.clone();
        drop(state);
        self.schedule_persist();
        Some(updated)
    }

    /// Delete delegated work item.
    pub async fn delete_delegated_work(&self, item_id: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.delegated_work.remove(item_id).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }

    // ── Workspace Snapshots ─────────────────────────────────────────

    /// List all saved workspace snapshots.
    pub async fn list_workspaces(&self) -> Vec<WorkspaceSnapshot> {
        let state = self.inner.read().await;
        let mut list: Vec<WorkspaceSnapshot> = state.workspaces.values().cloned().collect();
        // Sort by creation time (newest first).
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    /// Save (upsert) a workspace snapshot.
    pub async fn save_workspace(&self, snapshot: WorkspaceSnapshot) {
        let mut state = self.inner.write().await;
        state
            .workspaces
            .insert(snapshot.name.clone(), snapshot);
        drop(state);
        self.schedule_persist();
    }

    /// Delete a workspace snapshot by name. Returns true if it existed.
    pub async fn delete_workspace(&self, name: &str) -> bool {
        let mut state = self.inner.write().await;
        let removed = state.workspaces.remove(name).is_some();
        drop(state);
        if removed {
            self.schedule_persist();
        }
        removed
    }
}
