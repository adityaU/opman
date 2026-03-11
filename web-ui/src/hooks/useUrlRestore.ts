import { useState, useCallback, useEffect, useRef } from "react";
import { useUrlState, readUrlState } from "./useUrlState";
import type { UrlState } from "./useUrlState";
import { switchProject, selectSession } from "../api";

export interface UseUrlRestoreOptions {
  appState: any;
  activeSessionId: string | null;
  panels: {
    sidebarOpen: boolean;
    terminalOpen: boolean;
    neovimOpen: boolean;
    gitOpen: boolean;
  };
  setPanels: (p: { sidebar: boolean; terminal: boolean; editor: boolean; git: boolean }) => void;
  refreshState: () => void;
}

/**
 * Reads the initial URL state, restores session from URL on first load,
 * and keeps URL in sync with app state.
 */
export function useUrlRestore(opts: UseUrlRestoreOptions) {
  const { appState, activeSessionId, panels, setPanels, refreshState } = opts;

  const [initialUrlState] = useState(() => readUrlState());
  const urlRestoredRef = useRef(false);

  // ── Restore session from URL on first app state load ──
  // We retry on every appState update until sessions are populated,
  // because the backend may not have hydrated sessions on the first fetch.
  useEffect(() => {
    if (!appState || urlRestoredRef.current) return;

    const urlSid = initialUrlState.sessionId;
    const urlProjIdx = initialUrlState.projectIdx;

    // Nothing to restore from URL — try localStorage fallback
    if (!urlSid) {
      // Only give up once sessions have actually loaded (non-empty)
      const proj = appState.projects[appState.active_project];
      if (proj && proj.sessions.length > 0 && proj.active_session) {
        urlRestoredRef.current = true;
      } else if (proj && proj.sessions.length > 0 && !proj.active_session) {
        // Sessions loaded but backend didn't set active — try localStorage
        const lastSid = localStorage.getItem("opman_last_session");
        if (lastSid && proj.sessions.some((s: any) => s.id === lastSid)) {
          urlRestoredRef.current = true;
          (async () => {
            try {
              await selectSession(appState.active_project, lastSid);
              refreshState();
            } catch { /* ignore */ }
          })();
        } else {
          urlRestoredRef.current = true;
        }
      }
      // If sessions are still empty, don't set the ref — we'll retry
      return;
    }

    const currentSid = appState.projects[appState.active_project]?.active_session;
    if (currentSid === urlSid) { urlRestoredRef.current = true; return; }

    // Check if any project has the URL session in its session list
    let targetProject = urlProjIdx;
    if (targetProject === null) {
      for (let i = 0; i < appState.projects.length; i++) {
        if (appState.projects[i].sessions.some((s: any) => s.id === urlSid)) {
          targetProject = i;
          break;
        }
      }
    }
    // Sessions not yet populated — wait for next appState update
    if (targetProject === null) return;

    urlRestoredRef.current = true;
    (async () => {
      try {
        if (targetProject !== appState.active_project) {
          await switchProject(targetProject!);
        }
        await selectSession(targetProject!, urlSid);
        refreshState();
      } catch {
        // Silently ignore — URL might have a stale session
      }
    })();
  }, [appState, initialUrlState, refreshState]);

  // ── Persist last active session to localStorage for cross-restart restore ──
  useEffect(() => {
    if (activeSessionId) {
      localStorage.setItem("opman_last_session", activeSessionId);
    }
  }, [activeSessionId]);

  // ── Handle popstate ──
  const handlePopState = useCallback(
    (state: UrlState) => {
      setPanels(state.panels);

      if (state.sessionId && appState) {
        const currentSid = appState.projects[appState.active_project]?.active_session;
        if (currentSid !== state.sessionId) {
          const projIdx = state.projectIdx ?? appState.active_project;
          (async () => {
            try {
              if (projIdx !== appState.active_project) {
                await switchProject(projIdx);
              }
              await selectSession(projIdx, state.sessionId!);
              refreshState();
            } catch {
              // ignore
            }
          })();
        }
      }
    },
    [appState, refreshState, setPanels],
  );

  // ── Keep URL in sync ──
  useUrlState({
    sessionId: activeSessionId,
    projectIdx: appState?.active_project ?? 0,
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
