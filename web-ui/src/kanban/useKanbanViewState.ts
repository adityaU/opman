import { useState, useCallback, useEffect } from "react";
import { appNavigate, onLocationChange } from "../utils/navigation";

/** Path-based route for the Kanban board. The board is its own destination
 *  (`/kanban`), mutually exclusive with the chat view by pathname — not a
 *  `?view=` flag layered on the session URL. The optional `?project=` selects
 *  the board's project and `?task=` deep-links a task's editor. */
export interface KanbanViewState {
  /** True when the current path is the Kanban board. */
  isKanbanView: boolean;
  /** Task to focus/open when the board mounts (`?task=<id>`), else null. */
  focusTaskId: string | null;
  /** Navigate to the board for a project (drops any prior task focus). */
  openKanban: (projectIndex?: number) => void;
  /** Navigate to the board and open a specific task's editor. */
  openKanbanTask: (taskId: string, projectIndex?: number) => void;
  /** Drop `?task` once the board has consumed it (no new history entry). */
  clearFocusTask: () => void;
}

export const KANBAN_PATH = "/kanban";

function readView(): boolean {
  return window.location.pathname.startsWith(KANBAN_PATH);
}

function readTask(): string | null {
  return new URLSearchParams(window.location.search).get("task");
}

/** Build a `/kanban` URL with optional project + task params. */
function kanbanUrl(projectIndex?: number, taskId?: string): string {
  const params = new URLSearchParams();
  if (projectIndex != null) params.set("project", String(projectIndex));
  if (taskId) params.set("task", taskId);
  const qs = params.toString();
  return qs ? `${KANBAN_PATH}?${qs}` : KANBAN_PATH;
}

export function useKanbanViewState(): KanbanViewState {
  const [isKanbanView, setIsKanbanView] = useState<boolean>(readView);
  const [focusTaskId, setFocusTaskId] = useState<string | null>(readTask);

  const openKanban = useCallback((projectIndex?: number) => {
    appNavigate(kanbanUrl(projectIndex));
  }, []);

  const openKanbanTask = useCallback((taskId: string, projectIndex?: number) => {
    appNavigate(kanbanUrl(projectIndex, taskId));
  }, []);

  const clearFocusTask = useCallback(() => {
    setFocusTaskId(null);
    if (!readView()) return;
    const params = new URLSearchParams(window.location.search);
    if (!params.has("task")) return;
    params.delete("task");
    const qs = params.toString();
    appNavigate(qs ? `${KANBAN_PATH}?${qs}` : KANBAN_PATH, { replace: true });
  }, []);

  // Recompute from the URL on any navigation — back/forward (popstate) or a
  // programmatic appNavigate from another hook (e.g. selecting a session,
  // which leaves the board). Keeps view state in lockstep with the path.
  useEffect(() => {
    return onLocationChange(() => {
      setIsKanbanView(readView());
      setFocusTaskId(readTask());
    });
  }, []);

  return { isKanbanView, focusTaskId, openKanban, openKanbanTask, clearFocusTask };
}
