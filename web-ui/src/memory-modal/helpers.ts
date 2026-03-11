import type { MemoryScope, PersonalMemoryItem, ProjectInfo } from "../api";

export function formatScope(scope: MemoryScope): string {
  switch (scope) {
    case "global":
      return "Global";
    case "project":
      return "Project";
    case "session":
      return "Session";
  }
}

export function describeScope(item: PersonalMemoryItem, projects: ProjectInfo[]): string {
  if (item.scope === "global") return "All work";
  if (item.scope === "project") return projects[item.project_index ?? -1]?.name ?? "Project scope";
  return item.session_id ? `Session ${item.session_id.slice(0, 8)}` : "Session scope";
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
