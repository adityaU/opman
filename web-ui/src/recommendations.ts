import type { AutonomyMode, DelegatedWorkItem, Mission, PersonalMemoryItem, RoutineDefinition, WorkspaceSnapshot } from "./api";
import type { PermissionRequest, QuestionRequest } from "./types";

export type RecommendationAction =
  | "open_inbox"
  | "open_missions"
  | "open_memory"
  | "open_routines"
  | "open_delegation"
  | "open_workspaces"
  | "open_autonomy"
  | "setup_daily_summary"
  | "upgrade_autonomy_nudge"
  | "setup_daily_copilot";

export interface AssistantRecommendation {
  id: string;
  title: string;
  rationale: string;
  action: RecommendationAction;
  priority: "high" | "medium" | "low";
}

interface BuildRecommendationsArgs {
  autonomyMode: AutonomyMode;
  missions: Mission[];
  delegatedWork: DelegatedWorkItem[];
  memoryItems: PersonalMemoryItem[];
  routines: RoutineDefinition[];
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  workspaces: WorkspaceSnapshot[];
}

export function buildRecommendations({
  autonomyMode,
  missions,
  delegatedWork,
  memoryItems,
  routines,
  permissions,
  questions,
  workspaces,
}: BuildRecommendationsArgs): AssistantRecommendation[] {
  const recommendations: AssistantRecommendation[] = [];

  const blockedMissions = missions.filter((item) => item.status === "blocked");
  const pendingApprovals = permissions.length + questions.length;
  const incompleteDelegation = delegatedWork.filter((item) => item.status !== "completed");
  const scheduledRoutines = routines.filter((item) => item.trigger === "daily_summary");
  const recipes = workspaces.filter((item) => item.is_recipe);

  if (scheduledRoutines.length === 0 && autonomyMode === "observe") {
    recommendations.push({
      id: "daily-copilot-preset",
      title: "Enable Daily Copilot",
      rationale: "Set up a daily briefing, nudge-level autonomy, and a reusable morning workflow in one step.",
      action: "setup_daily_copilot",
      priority: "high",
    });
  }

  if (pendingApprovals > 0) {
    recommendations.push({
      id: "needs-you-inbox",
      title: "Clear assistant blockers",
      rationale: `${pendingApprovals} pending approval/question item${pendingApprovals === 1 ? "" : "s"} are blocking progress.`,
      action: "open_inbox",
      priority: "high",
    });
  }

  if (blockedMissions.length > 0) {
    recommendations.push({
      id: "blocked-missions",
      title: "Unblock mission flow",
      rationale: `${blockedMissions.length} mission${blockedMissions.length === 1 ? " is" : "s are"} marked blocked and likely need a next action update.`,
      action: "open_missions",
      priority: "high",
    });
  }

  if (memoryItems.length === 0) {
    recommendations.push({
      id: "seed-memory",
      title: "Teach your assistant your preferences",
      rationale: "Add persistent memory so future prompts and summaries become more personalized.",
      action: "open_memory",
      priority: "medium",
    });
  }

  if (scheduledRoutines.length === 0) {
    recommendations.push({
      id: "schedule-summary",
      title: "Set up a daily summary",
      rationale: "A scheduled summary makes opman feel proactive even when you are away.",
      action: "setup_daily_summary",
      priority: "medium",
    });
  }

  if (incompleteDelegation.length > 2) {
    recommendations.push({
      id: "delegation-overview",
      title: "Review delegated work load",
      rationale: `${incompleteDelegation.length} delegated work item${incompleteDelegation.length === 1 ? " is" : "s are"} still in flight.`,
      action: "open_delegation",
      priority: "medium",
    });
  }

  if (recipes.length === 0) {
    recommendations.push({
      id: "save-recipe",
      title: "Capture a reusable workspace recipe",
      rationale: "Recipes turn recurring workflows into one-click launches.",
      action: "open_workspaces",
      priority: "low",
    });
  }

  if (autonomyMode === "observe") {
    recommendations.push({
      id: "raise-autonomy",
      title: "Enable more proactive assistance",
      rationale: "Switching from Observe to Nudge or Continue unlocks more assistant behavior.",
      action: "upgrade_autonomy_nudge",
      priority: "low",
    });
  }

  return recommendations.slice(0, 4);
}
