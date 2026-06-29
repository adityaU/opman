import { useState, useCallback, useEffect, useRef } from "react";
import { useUrlState, readUrlState } from "./useUrlState";
import type { UrlState } from "./useUrlState";
import { KANBAN_PATH } from "../kanban/useKanbanViewState";

export interface UseUrlRestoreOptions {
  appState: any;
  activeSessionId: string | null;
  /** Active project index (from URL session state). */
  activeProjectIndex: number;
  panels: {
    sidebarOpen: boolean;
    terminalOpen: boolean;
    neovimOpen: boolean;
    gitOpen: boolean;
  };
  setPanels: (p: { sidebar: boolean; terminal: boolean; editor: boolean; git: boolean }) => void;
  /** Navigate to a session via URL — single source of truth. */
  setUrlSession: (sessionId: string, projectIdx: number) => void;
}

/**
 * Handles:
 * 1. localStorage fallback when URL has no session (cross-restart restore)
 * 2. Persisting activeSessionId to localStorage
 * 3. URL ↔ app state sync (via useUrlState)
 * 4. Panel restoration on popstate (session popstate handled by useUrlSessionState)
 */
export function useUrlRestore(opts: UseUrlRestoreOptions) {
  const { appState, activeSessionId, activeProjectIndex, panels, setPanels, setUrlSession } = opts;

  const [initialUrlState] = useState(() => readUrlState());
  const urlRestoredRef = useRef(false);

  // ── localStorage fallback when URL has no session ──
  // If the URL didn't contain a session, try restoring from localStorage.
  useEffect(() => {
    if (!appState || urlRestoredRef.current) return;

    // Don't auto-restore a session when the app was loaded on the Kanban route —
    // setUrlSession would navigate to "/" and bounce the user off the board.
    if (window.location.pathname.startsWith(KANBAN_PATH)) {
      urlRestoredRef.current = true;
      return;
    }

    // Only run when URL has no session to restore
    if (initialUrlState.sessionId) {
      urlRestoredRef.current = true;
      return;
    }

    // Use the URL-derived project index (already defaults to 0 when absent).
    const projIdx = activeProjectIndex;
    const proj = appState.projects[projIdx];
    if (!proj || proj.sessions.length === 0) return; // sessions not yet loaded

    if (proj.active_session) {
      // Backend already has an active session — write it to the URL so
      // urlSessionId is populated.
      urlRestoredRef.current = true;
      setUrlSession(proj.active_session, projIdx);
      return;
    }

    // Sessions loaded but backend didn't set active — try localStorage
    const lastSid = localStorage.getItem("opman_last_session");
    if (lastSid && proj.sessions.some((s: any) => s.id === lastSid)) {
      urlRestoredRef.current = true;
      setUrlSession(lastSid, projIdx);
    } else {
      urlRestoredRef.current = true;
    }
  }, [appState, initialUrlState.sessionId, setUrlSession]);

  // ── Persist last active session to localStorage for cross-restart restore ──
  useEffect(() => {
    if (activeSessionId) {
      localStorage.setItem("opman_last_session", activeSessionId);
    }
  }, [activeSessionId]);

  // ── Handle popstate (panels only — session handled by useUrlSessionState) ──
  const handlePopState = useCallback(
    (state: UrlState) => {
      setPanels(state.panels);
    },
    [setPanels],
  );

  // ── Keep URL in sync ──
  useUrlState({
    sessionId: activeSessionId,
    projectIdx: activeProjectIndex,
    panels: {
      sidebar: panels.sidebarOpen,
      terminal: panels.terminalOpen,
      editor: panels.neovimOpen,
      git: panels.gitOpen,
    },
    onPopState: handlePopState,
  });

  return { initialUrlState };
}
