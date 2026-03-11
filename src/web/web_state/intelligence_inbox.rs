//! Backend intelligence: inbox aggregation.
//!
//! The inbox now focuses on permissions, questions, watchers, and signals.
//! Mission-specific inbox items have been removed — missions now manage
//! their own lifecycle via the loop engine.

use super::super::types::*;

impl super::WebStateHandle {
    /// Build a unified, priority-sorted inbox from all sources.
    pub async fn build_inbox(&self, req: InboxRequest) -> Vec<InboxItem> {
        let mut items = Vec::new();

        // 1. Permissions → high-priority, unresolved
        for p in &req.permissions {
            items.push(InboxItem {
                id: format!("inbox-perm-{}", p.id),
                source: InboxItemSource::Permission,
                title: format!("Permission needed: {}", p.tool_name),
                description: p
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("{} wants to use {}", p.session_id, p.tool_name)),
                priority: InboxItemPriority::High,
                state: InboxItemState::Unresolved,
                created_at: p.time,
                session_id: Some(p.session_id.clone()),
                mission_id: None,
            });
        }

        // 2. Questions → high-priority, unresolved
        for q in &req.questions {
            items.push(InboxItem {
                id: format!("inbox-q-{}", q.id),
                source: InboxItemSource::Question,
                title: format!("Question: {}", q.title),
                description: format!("Session {} needs your input", q.session_id),
                priority: InboxItemPriority::High,
                state: InboxItemState::Unresolved,
                created_at: q.time,
                session_id: Some(q.session_id.clone()),
                mission_id: None,
            });
        }

        // 3. Watcher triggered → medium-priority, informational
        if let Some(ref ws) = req.watcher_status {
            if ws.action == "triggered" {
                items.push(InboxItem {
                    id: format!("inbox-watcher-{}", ws.session_id),
                    source: InboxItemSource::Watcher,
                    title: "Watcher triggered".to_string(),
                    description: format!(
                        "Session {} watcher fired",
                        &ws.session_id[..ws.session_id.len().min(8)]
                    ),
                    priority: InboxItemPriority::Medium,
                    state: InboxItemState::Informational,
                    created_at: chrono::Utc::now().timestamp_millis() as f64,
                    session_id: Some(ws.session_id.clone()),
                    mission_id: None,
                });
            }
        }

        // 4. Signals → informational
        for s in &req.signals {
            let priority = if s.kind == "watcher_trigger" {
                InboxItemPriority::Medium
            } else {
                InboxItemPriority::Low
            };
            items.push(InboxItem {
                id: format!("inbox-signal-{}", s.id),
                source: InboxItemSource::Completion,
                title: s.title.clone(),
                description: s.body.clone(),
                priority,
                state: InboxItemState::Informational,
                created_at: s.created_at,
                session_id: s.session_id.clone(),
                mission_id: None,
            });
        }

        // Sort: high(0) < medium(1) < low(2), then by created_at desc
        items.sort_by(|a, b| {
            let pa = priority_ord(&a.priority);
            let pb = priority_ord(&b.priority);
            pa.cmp(&pb)
                .then_with(|| b.created_at.partial_cmp(&a.created_at).unwrap_or(std::cmp::Ordering::Equal))
        });

        items
    }
}

fn priority_ord(p: &InboxItemPriority) -> u8 {
    match p {
        InboxItemPriority::High => 0,
        InboxItemPriority::Medium => 1,
        InboxItemPriority::Low => 2,
    }
}
