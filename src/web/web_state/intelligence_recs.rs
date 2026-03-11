//! Backend intelligence: recommendations engine.
//!
//! Mirrors the logic previously in `web-ui/src/recommendations.ts`.

use super::super::types::*;

impl super::WebStateHandle {
    /// Compute prioritised assistant recommendations.
    ///
    /// Reads missions, memory, routines, delegation, workspaces, and autonomy
    /// from its own state. Transient permissions/questions are passed in.
    pub async fn build_recommendations(
        &self,
        req: RecommendationsRequest,
    ) -> Vec<AssistantRecommendation> {
        let missions = self.list_missions().await;
        let memory = self.list_personal_memory().await;
        let (routines, _runs) = self.list_routines().await;
        let delegated = self.list_delegated_work().await;
        let workspaces = self.list_workspaces().await;
        let autonomy = self.get_autonomy_settings().await;

        let mut recs: Vec<AssistantRecommendation> = Vec::new();
        let mut next_id = 1u32;

        let has_daily_summary_routine = routines.iter().any(|r| {
            matches!(r.trigger, RoutineTrigger::DailySummary)
        });
        let is_observe = matches!(autonomy.mode, AutonomyMode::Observe);

        // 1. No daily summary + observe → high: enable daily copilot
        if !has_daily_summary_routine && is_observe {
            recs.push(AssistantRecommendation {
                id: format!("rec-{}", next_id),
                title: "Enable Daily Copilot".to_string(),
                rationale: "Set up a daily summary routine and move beyond observe mode \
                            for proactive assistance."
                    .to_string(),
                action: RecommendationAction::SetupDailyCopilot,
                priority: InboxItemPriority::High,
            });
            next_id += 1;
        }

        // 2. Pending blockers → high: clear blockers
        let blocker_count = req.permissions.len() + req.questions.len();
        if blocker_count > 0 {
            recs.push(AssistantRecommendation {
                id: format!("rec-{}", next_id),
                title: "Clear assistant blockers".to_string(),
                rationale: format!(
                    "{} pending permission/question request(s) need attention.",
                    blocker_count
                ),
                action: RecommendationAction::OpenInbox,
                priority: InboxItemPriority::High,
            });
            next_id += 1;
        }

        // 3. Blocked missions → high
        let blocked_count = missions
            .iter()
            .filter(|m| matches!(m.status, MissionStatus::Blocked))
            .count();
        if blocked_count > 0 {
            recs.push(AssistantRecommendation {
                id: format!("rec-{}", next_id),
                title: "Unblock mission flow".to_string(),
                rationale: format!(
                    "{} mission(s) are blocked and need your intervention.",
                    blocked_count
                ),
                action: RecommendationAction::OpenMissions,
                priority: InboxItemPriority::High,
            });
            next_id += 1;
        }

        // 4. No memory → medium: teach assistant
        if memory.is_empty() {
            recs.push(AssistantRecommendation {
                id: format!("rec-{}", next_id),
                title: "Teach your assistant".to_string(),
                rationale: "Add personal memory items so the assistant can recall your \
                            preferences and project context."
                    .to_string(),
                action: RecommendationAction::OpenMemory,
                priority: InboxItemPriority::Medium,
            });
            next_id += 1;
        }

        // 5. No daily summary routine → medium
        if !has_daily_summary_routine {
            recs.push(AssistantRecommendation {
                id: format!("rec-{}", next_id),
                title: "Set up a daily summary".to_string(),
                rationale: "A daily summary routine keeps you informed about session \
                            activity without manual checking."
                    .to_string(),
                action: RecommendationAction::SetupDailySummary,
                priority: InboxItemPriority::Medium,
            });
            next_id += 1;
        }

        // 6. Delegation overload → medium
        let incomplete_delegations = delegated
            .iter()
            .filter(|d| !matches!(d.status, DelegationStatus::Completed))
            .count();
        if incomplete_delegations > 2 {
            recs.push(AssistantRecommendation {
                id: format!("rec-{}", next_id),
                title: "Review delegated work load".to_string(),
                rationale: format!(
                    "{} delegated items are still in progress — consider reassigning or \
                     completing some.",
                    incomplete_delegations
                ),
                action: RecommendationAction::OpenDelegation,
                priority: InboxItemPriority::Medium,
            });
            next_id += 1;
        }

        // 7. No recipe workspaces → low
        let has_recipe = workspaces.iter().any(|w| w.is_recipe);
        if !has_recipe {
            recs.push(AssistantRecommendation {
                id: format!("rec-{}", next_id),
                title: "Capture a reusable workspace recipe".to_string(),
                rationale: "Save a workspace as a recipe to quickly recreate your \
                            preferred layout for specific tasks."
                    .to_string(),
                action: RecommendationAction::OpenWorkspaces,
                priority: InboxItemPriority::Low,
            });
            next_id += 1;
        }

        // 8. Observe mode → low: upgrade
        if is_observe {
            recs.push(AssistantRecommendation {
                id: format!("rec-{}", next_id),
                title: "Enable more proactive assistance".to_string(),
                rationale: "Moving beyond observe mode lets the assistant nudge, \
                            continue sessions, or act autonomously."
                    .to_string(),
                action: RecommendationAction::UpgradeAutonomyNudge,
                priority: InboxItemPriority::Low,
            });
            let _ = next_id;
        }

        // Truncate to max 4
        recs.truncate(4);
        recs
    }
}
