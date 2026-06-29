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
  const { board, tasks, pipelines } = useKanbanBoard(projectIndex, projectPath);

  return useMemo(() => {
    const map = new Map<string, SessionTaskLink>();
    if (!board) return map;

    const lanes = new Map(board.lanes.map((l) => [l.id, l]));
    const titles = new Map(tasks.map((t) => [t.id, t.title]));
    const link = (taskId: string, taskTitle: string, laneId: string, sessionId: string) => {
      const lane = lanes.get(laneId);
      map.set(sessionId, {
        taskId,
        taskTitle,
        laneId,
        laneName: lane?.name ?? laneId,
        laneColor: lane?.color ?? "",
      });
    };

    for (const t of tasks) {
      if (t.session_id) link(t.id, t.title, t.lane_id, t.session_id);
    }
    // Pipeline mode: each stage runs in its own session — tag each to its lane.
    // Listed after tasks so a stage's own lane wins over the task's current lane.
    for (const run of pipelines) {
      const title = titles.get(run.task_id) ?? "";
      for (const stage of run.stages) {
        if (stage.session_id) link(run.task_id, title, stage.lane_id, stage.session_id);
      }
    }
    return map;
  }, [board, tasks, pipelines]);
}
