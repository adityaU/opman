import { useState, useCallback, useEffect } from "react";

/** URL-param-driven view toggle for the Kanban board (`?view=kanban`).
 *  Mirrors how session/project state lives in the URL — no router. */
export interface KanbanViewState {
  /** True when `?view=kanban` is present. */
  isKanbanView: boolean;
  /** Task to focus/open when the board mounts (`?task=<id>`), else null. */
  focusTaskId: string | null;
  /** Set/clear `?view=kanban` while preserving other params (pi/session). */
  setKanbanView: (on: boolean) => void;
  /** Open the board and request a specific task's editor (`?view=kanban&task=<id>`). */
  openKanbanTask: (taskId: string) => void;
  /** Drop `?task` once the board has consumed it (no new history entry). */
  clearFocusTask: () => void;
}

function readView(): boolean {
  return new URLSearchParams(window.location.search).get("view") === "kanban";
}

function readTask(): string | null {
  return new URLSearchParams(window.location.search).get("task");
}

export function useKanbanViewState(): KanbanViewState {
  const [isKanbanView, setIsKanbanView] = useState<boolean>(readView);
  const [focusTaskId, setFocusTaskId] = useState<string | null>(readTask);

  const setKanbanView = useCallback((on: boolean) => {
    const params = new URLSearchParams(window.location.search);
    if (on) {
      params.set("view", "kanban");
    } else {
      params.delete("view");
      params.delete("task");
    }
    const qs = params.toString();
    window.history.pushState(null, "", qs ? `?${qs}` : window.location.pathname);
    setIsKanbanView(on);
    if (!on) setFocusTaskId(null);
  }, []);

  const openKanbanTask = useCallback((taskId: string) => {
    const params = new URLSearchParams(window.location.search);
    params.set("view", "kanban");
    params.set("task", taskId);
    window.history.pushState(null, "", `?${params.toString()}`);
    setIsKanbanView(true);
    setFocusTaskId(taskId);
  }, []);

  const clearFocusTask = useCallback(() => {
    setFocusTaskId(null);
    const params = new URLSearchParams(window.location.search);
    if (!params.has("task")) return;
    params.delete("task");
    const qs = params.toString();
    window.history.replaceState(null, "", qs ? `?${qs}` : window.location.pathname);
  }, []);

  // React to browser back/forward.
  useEffect(() => {
    const handler = () => {
      setIsKanbanView(readView());
      setFocusTaskId(readTask());
    };
    window.addEventListener("popstate", handler);
    return () => window.removeEventListener("popstate", handler);
  }, []);

  return { isKanbanView, focusTaskId, setKanbanView, openKanbanTask, clearFocusTask };
}
