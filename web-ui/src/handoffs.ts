import type { Mission } from "./api";
import type { PermissionRequest, QuestionRequest } from "./types";
import type { ActivityEvent } from "./api";

export interface HandoffLink {
  kind: "mission" | "permission" | "question" | "activity";
  label: string;
  sourceId?: string;
}

export interface HandoffBrief {
  title: string;
  summary: string;
  blockers: string[];
  recentChanges: string[];
  nextAction: string;
  links: HandoffLink[];
}

interface BuildMissionHandoffArgs {
  mission: Mission;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  activityEvents: ActivityEvent[];
}

interface BuildSessionHandoffArgs {
  sessionId: string | null;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  activityEvents: ActivityEvent[];
}

export function buildMissionHandoff({
  mission,
  permissions,
  questions,
  activityEvents,
}: BuildMissionHandoffArgs): HandoffBrief {
  const linkedPermissions = permissions.filter((item) => item.sessionID === mission.session_id);
  const linkedQuestions = questions.filter((item) => item.sessionID === mission.session_id);
  const linkedActivity = activityEvents.filter((item) => item.session_id === mission.session_id).slice(-3).reverse();

  const blockers = [
    ...linkedPermissions.map((item) => `Permission required for ${item.toolName}`),
    ...linkedQuestions.map((item) => item.title || "Unanswered AI question"),
  ];

  if (mission.status === "blocked" && blockers.length === 0) {
    blockers.push("Mission is marked blocked and needs review.");
  }

  const recentChanges = linkedActivity.length
    ? linkedActivity.map((event) => event.summary)
    : [mission.goal];

  const nextAction = blockers.length > 0
    ? blockers[0]
    : mission.next_action || "Review the latest session activity and continue the mission.";

  const links: HandoffLink[] = [
    { kind: "mission", label: mission.title, sourceId: mission.id },
    ...linkedPermissions.map((item) => ({
      kind: "permission" as const,
      label: `Permission: ${item.toolName}`,
      sourceId: item.id,
    })),
    ...linkedQuestions.map((item) => ({
      kind: "question" as const,
      label: item.title || "Pending question",
      sourceId: item.id,
    })),
  ];

  return {
    title: mission.title,
    summary: mission.goal,
    blockers,
    recentChanges,
    nextAction,
    links,
  };
}

export function buildSessionHandoff({
  sessionId,
  permissions,
  questions,
  activityEvents,
}: BuildSessionHandoffArgs): HandoffBrief | null {
  if (!sessionId) return null;

  const linkedPermissions = permissions.filter((item) => item.sessionID === sessionId);
  const linkedQuestions = questions.filter((item) => item.sessionID === sessionId);
  const linkedActivity = activityEvents.filter((item) => item.session_id === sessionId).slice(-3).reverse();

  const blockers = [
    ...linkedPermissions.map((item) => `Permission required for ${item.toolName}`),
    ...linkedQuestions.map((item) => item.title || "Unanswered AI question"),
  ];

  return {
    title: `Session ${sessionId.slice(0, 8)}`,
    summary: linkedActivity[0]?.summary || "Recent session progress is available.",
    blockers,
    recentChanges: linkedActivity.length ? linkedActivity.map((item) => item.summary) : ["No recent activity yet."],
    nextAction: blockers[0] || "Resume the session and continue the latest task.",
    links: [
      ...linkedPermissions.map((item) => ({ kind: "permission" as const, label: `Permission: ${item.toolName}`, sourceId: item.id })),
      ...linkedQuestions.map((item) => ({ kind: "question" as const, label: item.title || "Pending question", sourceId: item.id })),
    ],
  };
}
