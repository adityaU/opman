import type { Mission } from "./api";
import type { PermissionRequest, QuestionRequest } from "./types";
import type { ActivityEvent } from "./api";
import type { AssistantSignal } from "./inbox";
import { buildMissionHandoff, buildSessionHandoff } from "./handoffs";

export interface ResumeBriefing {
  title: string;
  summary: string;
  nextAction: string;
}

interface BuildResumeBriefingArgs {
  activeSessionId: string | null;
  missions: Mission[];
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  activityEvents: ActivityEvent[];
  signals: AssistantSignal[];
}

export function buildResumeBriefing({
  activeSessionId,
  missions,
  permissions,
  questions,
  activityEvents,
  signals,
}: BuildResumeBriefingArgs): ResumeBriefing | null {
  const linkedMission = missions.find((mission) => mission.session_id === activeSessionId && mission.status !== "completed");
  const missionBrief = linkedMission
    ? buildMissionHandoff({ mission: linkedMission, permissions, questions, activityEvents })
    : null;
  const sessionBrief = buildSessionHandoff({ sessionId: activeSessionId, permissions, questions, activityEvents });
  const recentSignals = signals.slice(0, 2).map((signal) => signal.title);

  if (!missionBrief && !sessionBrief && recentSignals.length === 0) return null;

  const source = missionBrief ?? sessionBrief;
  if (source) {
    return {
      title: source.title,
      summary: [source.summary, ...recentSignals].filter(Boolean).join(" • "),
      nextAction: source.nextAction,
    };
  }

  return {
    title: "Resume briefing",
    summary: recentSignals.join(" • "),
    nextAction: "Open the inbox or missions to continue where you left off.",
  };
}
