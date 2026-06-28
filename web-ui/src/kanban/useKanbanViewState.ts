import { useState, useCallback, useEffect } from "react";

/** URL-param-driven view toggle for the Kanban board (`?view=kanban`).
 *  Mirrors how session/project state lives in the URL — no router. */
export interface KanbanViewState {
  /** True when `?view=kanban` is present. */
  isKanbanView: boolean;
  /** Set/clear `?view=kanban` while preserving other params (pi/session). */
  setKanbanView: (on: boolean) => void;
}

function readView(): boolean {
  return new URLSearchParams(window.location.search).get("view") === "kanban";
}

export function useKanbanViewState(): KanbanViewState {
  const [isKanbanView, setIsKanbanView] = useState<boolean>(readView);

  const setKanbanView = useCallback((on: boolean) => {
    const params = new URLSearchParams(window.location.search);
    if (on) params.set("view", "kanban");
    else params.delete("view");
    const qs = params.toString();
    window.history.pushState(null, "", qs ? `?${qs}` : window.location.pathname);
    setIsKanbanView(on);
  }, []);

  // React to browser back/forward.
  useEffect(() => {
    const handler = () => setIsKanbanView(readView());
    window.addEventListener("popstate", handler);
    return () => window.removeEventListener("popstate", handler);
  }, []);

  return { isKanbanView, setKanbanView };
}
