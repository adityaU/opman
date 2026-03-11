//! Backend intelligence: handoffs, resume briefing, daily summary.
//!
//! Mirrors logic from `web-ui/src/handoffs.ts`, `resumeBriefing.ts`,
//! and `dailySummary.ts`.

use super::super::types::*;

impl super::WebStateHandle {
    // ── Mission Handoff ─────────────────────────────────────────────

    /// Build a handoff brief for a specific mission.
    pub async fn build_mission_handoff(
        &self,
        req: MissionHandoffRequest,
    ) -> Option<HandoffBrief> {
        let missions = self.list_missions().await;
        let mission = missions.iter().find(|m| m.id == req.mission_id)?;

        let session_id = mission.session_id.as_deref().unwrap_or("");

        // Filter permissions/questions to this mission's session
        let session_perms: Vec<&PermissionInput> = req
            .permissions
            .iter()
            .filter(|p| p.session_id == session_id)
            .collect();
        let session_questions: Vec<&QuestionInput> = req
            .questions
            .iter()
            .filter(|q| q.session_id == session_id)
            .collect();

        // Activity events for context
        let activity = if !session_id.is_empty() {
            self.get_activity_feed(session_id).await
        } else {
            Vec::new()
        };
        let recent: Vec<&ActivityEventPayload> =
            activity.iter().rev().take(3).collect();

        // Blockers
        let mut blockers: Vec<String> = Vec::new();
        for p in &session_perms {
            blockers.push(format!("Permission needed: {}", p.tool_name));
        }
        for q in &session_questions {
            blockers.push(format!("Question pending: {}", q.title));
        }
        if matches!(mission.status, MissionStatus::Blocked) && blockers.is_empty() {
            blockers.push("Mission is marked blocked".to_string());
        }

        // Recent changes
        let recent_changes: Vec<String> = if recent.is_empty() {
            vec![mission.goal.clone()]
        } else {
            recent.iter().map(|e| e.summary.clone()).collect()
        };

        // Next action
        let next_action = if let Some(first_blocker) = blockers.first() {
            first_blocker.clone()
        } else if !mission.next_action.is_empty() {
            mission.next_action.clone()
        } else {
            "Continue working on this mission".to_string()
        };

        // Links
        let mut links = vec![HandoffLink {
            kind: "mission".to_string(),
            label: mission.title.clone(),
            source_id: Some(mission.id.clone()),
        }];
        for p in &session_perms {
            links.push(HandoffLink {
                kind: "permission".to_string(),
                label: p.tool_name.clone(),
                source_id: Some(p.id.clone()),
            });
        }
        for q in &session_questions {
            links.push(HandoffLink {
                kind: "question".to_string(),
                label: q.title.clone(),
                source_id: Some(q.id.clone()),
            });
        }

        Some(HandoffBrief {
            title: format!("Mission: {}", mission.title),
            summary: mission.goal.clone(),
            blockers,
            recent_changes,
            next_action,
            links,
        })
    }

    // ── Session Handoff ─────────────────────────────────────────────

    /// Build a handoff brief for a session (no specific mission).
    pub async fn build_session_handoff(
        &self,
        req: SessionHandoffRequest,
    ) -> Option<HandoffBrief> {
        if req.session_id.is_empty() {
            return None;
        }

        let sid = &req.session_id;
        let session_perms: Vec<&PermissionInput> = req
            .permissions
            .iter()
            .filter(|p| p.session_id == *sid)
            .collect();
        let session_questions: Vec<&QuestionInput> = req
            .questions
            .iter()
            .filter(|q| q.session_id == *sid)
            .collect();

        let activity = self.get_activity_feed(sid).await;
        let recent: Vec<&ActivityEventPayload> =
            activity.iter().rev().take(3).collect();

        let short_id = &sid[..sid.len().min(8)];

        let summary = recent
            .first()
            .map(|e| e.summary.clone())
            .unwrap_or_else(|| format!("Session {}", short_id));

        let mut blockers: Vec<String> = Vec::new();
        for p in &session_perms {
            blockers.push(format!("Permission needed: {}", p.tool_name));
        }
        for q in &session_questions {
            blockers.push(format!("Question pending: {}", q.title));
        }

        let recent_changes: Vec<String> = if recent.is_empty() {
            vec![summary.clone()]
        } else {
            recent.iter().map(|e| e.summary.clone()).collect()
        };

        let next_action = blockers
            .first()
            .cloned()
            .unwrap_or_else(|| "Continue session".to_string());

        let mut links = Vec::new();
        for p in &session_perms {
            links.push(HandoffLink {
                kind: "permission".to_string(),
                label: p.tool_name.clone(),
                source_id: Some(p.id.clone()),
            });
        }
        for q in &session_questions {
            links.push(HandoffLink {
                kind: "question".to_string(),
                label: q.title.clone(),
                source_id: Some(q.id.clone()),
            });
        }

        Some(HandoffBrief {
            title: format!("Session {}", short_id),
            summary,
            blockers,
            recent_changes,
            next_action,
            links,
        })
    }

    // ── Resume Briefing ─────────────────────────────────────────────

    /// Build a resume briefing for the current session.
    pub async fn build_resume_briefing(
        &self,
        req: ResumeBriefingRequest,
    ) -> Option<ResumeBriefing> {
        let active_sid = req.active_session_id.as_deref().unwrap_or("");
        let missions = self.list_missions().await;

        // Find the active mission for the current session
        let active_mission = missions.iter().find(|m| {
            m.session_id.as_deref() == Some(active_sid)
                && !matches!(m.status, MissionStatus::Completed)
        });

        // Build mission handoff if applicable
        let mission_brief = if let Some(m) = active_mission {
            self.build_mission_handoff(MissionHandoffRequest {
                mission_id: m.id.clone(),
                permissions: req.permissions.clone(),
                questions: req.questions.clone(),
            })
            .await
        } else {
            None
        };

        // Build session handoff
        let session_brief = self
            .build_session_handoff(SessionHandoffRequest {
                session_id: active_sid.to_string(),
                permissions: req.permissions.clone(),
                questions: req.questions.clone(),
            })
            .await;

        // Recent signal titles
        let recent_signals: Vec<&str> = req
            .signals
            .iter()
            .take(2)
            .map(|s| s.title.as_str())
            .collect();

        if mission_brief.is_none() && session_brief.is_none() && recent_signals.is_empty() {
            return None;
        }

        let source = mission_brief.as_ref().or(session_brief.as_ref());

        if let Some(src) = source {
            let signal_part = if recent_signals.is_empty() {
                String::new()
            } else {
                format!(" \u{2022} {}", recent_signals.join(" \u{2022} "))
            };
            Some(ResumeBriefing {
                title: src.title.clone(),
                summary: format!("{}{}", src.summary, signal_part),
                next_action: src.next_action.clone(),
            })
        } else {
            Some(ResumeBriefing {
                title: "Welcome back".to_string(),
                summary: recent_signals.join(" \u{2022} "),
                next_action: "Check your recent signals".to_string(),
            })
        }
    }

    // ── Daily Summary ───────────────────────────────────────────────

    /// Build a daily summary string for a routine.
    pub async fn build_daily_summary(
        &self,
        req: DailySummaryRequest,
    ) -> String {
        let missions = self.list_missions().await;
        let (routines, _) = self.list_routines().await;

        let routine = routines.iter().find(|r| r.id == req.routine_id);
        let routine_name = routine
            .map(|r| r.name.as_str())
            .unwrap_or("Daily Summary");

        let active_count = missions
            .iter()
            .filter(|m| matches!(m.status, MissionStatus::Active))
            .count();
        let blocked_count = missions
            .iter()
            .filter(|m| matches!(m.status, MissionStatus::Blocked))
            .count();
        let needs_you = req.permissions.len() + req.questions.len() + blocked_count;

        let signal_titles: Vec<&str> = req
            .signals
            .iter()
            .take(2)
            .map(|s| s.title.as_str())
            .collect();

        let mut parts = vec![format!(
            "{}: {} active missions",
            routine_name, active_count
        )];
        if needs_you > 0 {
            parts.push(format!("{} items need attention", needs_you));
        }
        if !signal_titles.is_empty() {
            parts.push(format!("recent: {}", signal_titles.join("; ")));
        }

        parts.join(" \u{2022} ")
    }
}
