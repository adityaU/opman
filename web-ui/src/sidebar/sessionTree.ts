/**
 * Pure shaping of a project's session list for the sidebar: parents vs subagent
 * children, and kanban-task grouping. Kept out of `ProjectNode` so the component is
 * rendering only.
 */
import type { SessionInfo } from "../api";
import type { TaskGroup } from "./TaskGroupNode";
import type { SessionTaskLink } from "./useSessionTaskLinks";

export interface SessionTree {
  parentSessions: SessionInfo[];
  childrenMap: Map<string, SessionInfo[]>;
  hasActive: boolean;
}

/** Top-level sessions (newest first), their children by parent id, and whether any is busy. */
export function splitSessionTree(
  sessions: SessionInfo[],
  isSessionBusy: (id: string) => boolean,
): SessionTree {
  const parents: SessionInfo[] = [];
  const children = new Map<string, SessionInfo[]>();
  for (const s of sessions) {
    if (!s.parentID) {
      parents.push(s);
      continue;
    }
    const list = children.get(s.parentID) || [];
    list.push(s);
    children.set(s.parentID, list);
  }
  // Recency only: pinned rows are lifted out by `groupSessions`, which holds them
  // above the date headings rather than sorting them to the top of one.
  parents.sort((a, b) => b.time.updated - a.time.updated);
  const hasActive = sessions.some((s) => isSessionBusy(s.id));
  return { parentSessions: parents, childrenMap: children, hasActive };
}

/**
 * Sessions linked to a kanban task, grouped under it, and the rest. The input is already
 * recency-sorted, so first-seen insertion keeps both the group order and each group's
 * order, and the first session seen carries the group's current lane colour. Sessions of
 * archived tasks are dropped from the sidebar entirely.
 */
export function groupByTask(
  parents: SessionInfo[],
  links: Map<string, SessionTaskLink> | undefined,
): { taskGroups: TaskGroup[]; ungroupedParents: SessionInfo[] } {
  if (!links || links.size === 0) return { taskGroups: [], ungroupedParents: parents };
  const byTask = new Map<string, TaskGroup>();
  const ungrouped: SessionInfo[] = [];
  for (const s of parents) {
    const link = links.get(s.id);
    if (!link) {
      ungrouped.push(s);
      continue;
    }
    if (link.archived) continue;
    let group = byTask.get(link.taskId);
    if (!group) {
      group = {
        taskId: link.taskId,
        taskTitle: link.taskTitle || s.title || s.id.slice(0, 12),
        laneColor: link.laneColor,
        lastUpdated: 0,
        sessions: [],
      };
      byTask.set(link.taskId, group);
    }
    group.sessions.push(s);
    if (s.time.updated > group.lastUpdated) group.lastUpdated = s.time.updated;
  }
  return { taskGroups: Array.from(byTask.values()), ungroupedParents: ungrouped };
}
