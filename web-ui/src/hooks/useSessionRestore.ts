import { useEffect, useRef } from "react";
import { onPageRoute } from "../utils/navigation";

/**
 * Which session to open on a cold start, and what to do when it is gone.
 *
 * The URL used to answer the first question — `?session=` was read on mount and
 * treated as the source of truth. It is localStorage's job alone now, which is
 * both simpler and more honest: a session id is not something the user typed
 * into the address bar, and a bookmarked one outlived the runner that owned it
 * often enough to need repairing anyway.
 *
 * That repair still has to happen, because the stored id has exactly the same
 * problem: switching runners or restarting one retires sessions, so the last
 * one is not guaranteed to still exist when the list finally loads.
 */

const LAST_SESSION_KEY = "opman_last_session";

export interface UseSessionRestoreOptions {
  readonly appState: any;
  readonly activeSessionId: string | null;
  readonly projectIndex: number;
  /** True while the user has explicitly asked for a new, not-yet-created session. */
  readonly newSessionMode: boolean;
  readonly selectSessionAt: (sessionId: string | null, projectIdx: number) => void;
}

export function useSessionRestore(opts: UseSessionRestoreOptions): void {
  const { appState, activeSessionId, projectIndex, newSessionMode, selectSessionAt } = opts;

  const restoredRef = useRef(false);
  const repairedRef = useRef(false);

  // ── Pick one on first load ──
  useEffect(() => {
    if (restoredRef.current || !appState) return;

    // Loaded straight onto a page of its own (/settings, /kanban): selecting a
    // session would navigate to "/" and bounce the user off the page they
    // asked for.
    if (onPageRoute()) {
      restoredRef.current = true;
      return;
    }

    // The user has already said what they want.
    if (activeSessionId || newSessionMode) {
      restoredRef.current = true;
      return;
    }

    const project = appState.projects?.[projectIndex];
    if (!project || project.sessions.length === 0) return; // list not in yet

    // The backend's own idea of the active session wins; it is the one the
    // runner is actually attached to.
    if (project.active_session) {
      restoredRef.current = true;
      selectSessionAt(project.active_session, projectIndex);
      return;
    }

    restoredRef.current = true;
    const stored = localStorage.getItem(LAST_SESSION_KEY);
    if (stored && project.sessions.some((s: any) => s.id === stored)) {
      selectSessionAt(stored, projectIndex);
    }
  }, [appState, activeSessionId, newSessionMode, projectIndex, selectSessionAt]);

  // ── Repair a restored session that no longer exists ──
  // Only the restored one: an explicit choice, or a session created a moment
  // ago that has not reached the list yet, must be left alone.
  useEffect(() => {
    if (repairedRef.current || !appState || !activeSessionId) return;

    const project = appState.projects?.[projectIndex];
    if (!project || project.sessions.length === 0) return;

    if (project.sessions.some((s: any) => s.id === activeSessionId)) {
      repairedRef.current = true;
      return;
    }

    const fallback = project.active_session;
    if (!fallback || !project.sessions.some((s: any) => s.id === fallback)) return;
    repairedRef.current = true;
    selectSessionAt(fallback, projectIndex);
  }, [appState, activeSessionId, projectIndex, selectSessionAt]);

  // ── Remember it for next time ──
  useEffect(() => {
    if (activeSessionId) localStorage.setItem(LAST_SESSION_KEY, activeSessionId);
  }, [activeSessionId]);
}
