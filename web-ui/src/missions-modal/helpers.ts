import type { MissionStatus } from "../api";

export function formatStatus(status: MissionStatus): string {
  switch (status) {
    case "planned":
      return "Planned";
    case "active":
      return "Active";
    case "blocked":
      return "Blocked";
    case "completed":
      return "Completed";
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
