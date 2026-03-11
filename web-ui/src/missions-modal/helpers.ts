import type { MissionState, EvalVerdict } from "../api";

export function formatState(state: MissionState): string {
  switch (state) {
    case "pending":
      return "Pending";
    case "executing":
      return "Executing";
    case "evaluating":
      return "Evaluating";
    case "paused":
      return "Paused";
    case "completed":
      return "Completed";
    case "cancelled":
      return "Cancelled";
    case "failed":
      return "Failed";
  }
}

export function stateColor(state: MissionState): string {
  switch (state) {
    case "executing":
    case "evaluating":
      return "var(--color-info, #5c8fff)";
    case "paused":
      return "var(--color-warning, #e6a817)";
    case "completed":
      return "var(--color-success, #4caf50)";
    case "failed":
    case "cancelled":
      return "var(--color-error, #e05252)";
    default:
      return "var(--color-text-muted, #999)";
  }
}

export function formatVerdict(verdict: EvalVerdict): string {
  switch (verdict) {
    case "achieved":
      return "Achieved";
    case "continue":
      return "Continue";
    case "blocked":
      return "Blocked";
    case "failed":
      return "Failed";
  }
}

export function formatRelativeDate(iso: string): string {
  try {
    const date = new Date(iso);
    return date.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

/** Whether a mission can accept a given action */
export function canPerformAction(
  state: MissionState,
  action: "start" | "pause" | "resume" | "cancel"
): boolean {
  switch (action) {
    case "start":
      return state === "pending";
    case "pause":
      return state === "executing" || state === "evaluating";
    case "resume":
      return state === "paused";
    case "cancel":
      return (
        state === "pending" ||
        state === "executing" ||
        state === "evaluating" ||
        state === "paused"
      );
  }
}
