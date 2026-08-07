//! Personal memory CRUD on the shared web state.

use chrono::Utc;

use super::super::types::*;
use super::uuid_like_id;

impl super::WebStateHandle {
    /// List all personal memory items.
    pub async fn list_personal_memory(&self) -> Vec<PersonalMemoryItem> {
        let state = self.inner.read().await;
        let mut list: Vec<PersonalMemoryItem> = state.personal_memory.values().cloned().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Create a personal memory item.
    pub async fn create_personal_memory(
        &self,
        req: CreatePersonalMemoryRequest,
    ) -> PersonalMemoryItem {
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
}

#[cfg(test)]
#[path = "assistant_memory_tests.rs"]
mod assistant_memory_tests;
