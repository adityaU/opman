//! Memory items filtered to the active project/session scope.

use super::super::types::*;

impl super::WebStateHandle {
    /// Return memory items filtered to the active scope.
    pub async fn list_active_memory(
        &self,
        project_index: Option<usize>,
        session_id: Option<&str>,
    ) -> Vec<PersonalMemoryItem> {
        let all = self.list_personal_memory().await;
        all.into_iter()
            .filter(|m| match m.scope {
                MemoryScope::Global => true,
                MemoryScope::Project => project_index.is_some() && m.project_index == project_index,
                MemoryScope::Session => {
                    session_id.is_some() && m.session_id.as_deref() == session_id
                }
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "active_memory_tests.rs"]
mod active_memory_tests;
