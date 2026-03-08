import type { Mission, RoutineDefinition } from "./api";
import type { PermissionRequest, QuestionRequest } from "./types";
import type { AssistantSignal } from "./inbox";

interface BuildDailySummaryArgs {
  routine: RoutineDefinition;
  missions: Mission[];
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  signals: AssistantSignal[];
}

export function buildDailySummary({
  routine,
  missions,
  permissions,
  questions,
  signals,
}: BuildDailySummaryArgs): string {
  const activeMissions = missions.filter((item) => item.status === "active").length;
  const blockedMissions = missions.filter((item) => item.status === "blocked").length;
  const needsYou = permissions.length + questions.length + blockedMissions;
  const recentSignals = signals.slice(0, 2).map((item) => item.title).join("; ");

  const parts = [
    `${routine.name}: ${activeMissions} active mission${activeMissions === 1 ? "" : "s"}`,
    `${needsYou} item${needsYou === 1 ? "" : "s"} need attention`,
  ];

  if (recentSignals) {
    parts.push(`recent: ${recentSignals}`);
  }

  return parts.join(" • ");
}

export function buildRoutineSummary(
  routine: RoutineDefinition,
  opts?: {
    activeSessionId?: string | null;
    missions?: Mission[];
    permissions?: PermissionRequest[];
    questions?: QuestionRequest[];
    signals?: AssistantSignal[];
  }
): string {
  if (routine.trigger === "daily_summary") {
    return buildDailySummary({
      routine,
      missions: opts?.missions ?? [],
      permissions: opts?.permissions ?? [],
      questions: opts?.questions ?? [],
      signals: opts?.signals ?? [],
    });
  }

  if (routine.trigger === "on_session_idle") {
    return `${routine.name}: auto-ran ${routine.action} for idle session ${opts?.activeSessionId?.slice(0, 8) ?? "unknown"}`;
  }

  return `${routine.name}: executed ${routine.action}`;
}
