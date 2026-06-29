import { useMemo } from "react";
import { useKanbanBoard } from "../kanban/useKanbanBoard";

/** Reverse link from a chat session back to the kanban task that launched it. */
export interface SessionTaskLink {
  taskId: string;
  taskTitle: string;
  laneId: string;
  laneName: string;
  /** Lane colour (hex) for tinting the sidebar tag; empty when unknown. */
  laneColor: string;
}

/** Build a `session_id → originating task/lane` map for the active project's
 *  board. The backend links Task → Session (task.session_id) but not the
 *  reverse, so the sidebar reconstructs it here to tag and back-link sessions
 *  that were launched from a kanban task.
 *
 *  Scoped to the active project — the only board we load — so we don't fetch
 *  every project's board on startup. Returns an empty map until the board loads
 *  (or when it fails), so callers degrade gracefully to no tags. */
export function useSessionTaskLinks(
  projectIndex: number,
  projectPath: string | undefined,
): Map<string, SessionTaskLink> {
  const { board, tasks } = useKanbanBoard(projectIndex, projectPath);

  return useMemo(() => {
    const map = new Map<string, SessionTaskLink>();
    if (!board) return map;

    const lanes = new Map(board.lanes.map((l) => [l.id, l]));
    for (const t of tasks) {
      if (!t.session_id) continue;
      const lane = lanes.get(t.lane_id);
      map.set(t.session_id, {
        taskId: t.id,
        taskTitle: t.title,
        laneId: t.lane_id,
        laneName: lane?.name ?? t.lane_id,
        laneColor: lane?.color ?? "",
      });
    }
    return map;
  }, [board, tasks]);
}
