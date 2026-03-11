//! Backend intelligence: handoffs, resume briefing, daily summary.
//!
//! Mission handoff has been removed — the new mission model handles its own
//! loop lifecycle. Session handoffs and resume briefing remain.

use super::super::types::*;

impl super::WebStateHandle {
    // ── Session Handoff ─────────────────────────────────────────────

    /// Build a handoff brief for a session.
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

        // Build session handoff
        let session_brief = self
            .build_session_handoff(SessionHandoffRequest {
                session_id: active_sid.to_string(),
                permissions: req.permissions.clone(),
                questions: req.questions.clone(),
            })
            .await;

        // Check for active missions on this session
        let missions = self.list_missions().await;
        let active_mission = missions.iter().find(|m| {
            m.session_id == active_sid
                && matches!(
                    m.state,
                    MissionState::Executing | MissionState::Evaluating | MissionState::Paused
                )
        });

        let mission_context = active_mission.map(|m| {
            let state_str = match m.state {
                MissionState::Executing => "executing",
                MissionState::Evaluating => "evaluating",
                MissionState::Paused => "paused",
                _ => "active",
            };
            format!(
                "Mission ({state_str}, iteration {}/{}): {}",
                m.iteration,
                if m.max_iterations == 0 { "∞".to_string() } else { m.max_iterations.to_string() },
                m.goal
            )
        });

        // Recent signal titles
        let recent_signals: Vec<&str> = req
            .signals
            .iter()
            .take(2)
            .map(|s| s.title.as_str())
            .collect();

        if session_brief.is_none() && mission_context.is_none() && recent_signals.is_empty() {
            return None;
        }

        if let Some(src) = session_brief.as_ref() {
            let mut summary = src.summary.clone();
            if let Some(ref mc) = mission_context {
                summary = format!("{} • {}", mc, summary);
            }
            let signal_part = if recent_signals.is_empty() {
                String::new()
            } else {
                format!(" \u{2022} {}", recent_signals.join(" \u{2022} "))
            };
            Some(ResumeBriefing {
                title: src.title.clone(),
                summary: format!("{}{}", summary, signal_part),
                next_action: src.next_action.clone(),
            })
        } else if let Some(mc) = mission_context {
            Some(ResumeBriefing {
                title: "Active mission".to_string(),
                summary: mc,
                next_action: "Check mission progress".to_string(),
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
            .filter(|m| matches!(m.state, MissionState::Executing | MissionState::Evaluating))
            .count();
        let needs_you = req.permissions.len() + req.questions.len();

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
