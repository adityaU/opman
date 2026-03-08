import type { Mission } from "./api";
import type { PermissionRequest, QuestionRequest } from "./types";
import type { WatcherStatus } from "./hooks/useSSE";

export interface AssistantSignal {
  id: string;
  kind: "session_complete" | "watcher_trigger";
  title: string;
  body: string;
  createdAt: number;
  sessionId?: string | null;
}

export type InboxItemPriority = "high" | "medium" | "low";
export type InboxItemState = "unresolved" | "informational";
export type InboxItemSource = "permission" | "question" | "mission" | "watcher" | "completion";

export interface InboxItem {
  id: string;
  source: InboxItemSource;
  title: string;
  description: string;
  priority: InboxItemPriority;
  state: InboxItemState;
  createdAt: number;
  sessionId?: string | null;
  missionId?: string;
  permission?: PermissionRequest;
  question?: QuestionRequest;
  signal?: AssistantSignal;
  mission?: Mission;
}

interface BuildInboxItemsArgs {
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  missions: Mission[];
  watcherStatus: WatcherStatus | null;
  signals: AssistantSignal[];
}

export function buildInboxItems({
  permissions,
  questions,
  missions,
  watcherStatus,
  signals,
}: BuildInboxItemsArgs): InboxItem[] {
  const items: InboxItem[] = [];

  for (const permission of permissions) {
    items.push({
      id: `permission:${permission.id}`,
      source: "permission",
      title: `Permission required: ${permission.toolName}`,
      description: permission.description || "AI needs approval to continue.",
      priority: "high",
      state: "unresolved",
      createdAt: permission.time,
      sessionId: permission.sessionID,
      permission,
    });
  }

  for (const question of questions) {
    items.push({
      id: `question:${question.id}`,
      source: "question",
      title: question.title || "AI has a question",
      description: `${question.questions.length} pending prompt${question.questions.length !== 1 ? "s" : ""}`,
      priority: "high",
      state: "unresolved",
      createdAt: question.time,
      sessionId: question.sessionID,
      question,
    });
  }

  for (const mission of missions.filter((entry) => entry.status === "blocked")) {
    items.push({
      id: `mission:${mission.id}`,
      source: "mission",
      title: mission.title,
      description: mission.next_action || mission.goal,
      priority: "high",
      state: "unresolved",
      createdAt: Date.parse(mission.updated_at) || Date.now(),
      sessionId: mission.session_id,
      missionId: mission.id,
      mission,
    });
  }

  if (watcherStatus && watcherStatus.action === "triggered") {
    items.push({
      id: `watcher:${watcherStatus.session_id}:${watcherStatus.action}`,
      source: "watcher",
      title: "Watcher triggered",
      description: "A watched session auto-continued and may need review.",
      priority: "medium",
      state: "informational",
      createdAt: Date.now(),
      sessionId: watcherStatus.session_id,
    });
  }

  for (const signal of signals) {
    items.push({
      id: `signal:${signal.id}`,
      source: signal.kind === "watcher_trigger" ? "watcher" : "completion",
      title: signal.title,
      description: signal.body,
      priority: signal.kind === "watcher_trigger" ? "medium" : "low",
      state: "informational",
      createdAt: signal.createdAt,
      sessionId: signal.sessionId,
      signal,
    });
  }

  return items.sort((a, b) => {
    const priorityOrder = { high: 0, medium: 1, low: 2 };
    return priorityOrder[a.priority] - priorityOrder[b.priority] || b.createdAt - a.createdAt;
  });
}
