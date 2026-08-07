import { useState, useCallback, useRef, useEffect } from "react";
import { selectSession, switchProject } from "../api";
import { appNavigate, onPageRoute } from "../utils/navigation";

/**
 * Which session and project are active, and the one way to change them.
 *
 * This used to keep both in `?session=&project=` and read the URL back as the
 * source of truth. It no longer does: the workspace persists its own layout,
 * the panels persist theirs, and a query string that described neither was a
 * second place for the same facts to live — one that could disagree with the
 * reducer after a back gesture, and that survived reloads pointing at sessions
 * the runner had since dropped.
 *
 * What the URL did carry that mattered is kept: selecting a session is still a
 * chat destination, so it navigates to `/` and takes the user off `/kanban` or
 * `/settings`. The path still routes; only the query string is gone.
 */

// ── Types ───────────────────────────────────────────────────

export interface SessionSelection {
  /** Active session id, or null before one has been chosen. */
  readonly sessionId: string | null;
  /** Active project index. */
  readonly projectIndex: number;
  /** True while the user has asked for a new session that does not exist yet. */
  readonly newSessionMode: boolean;
  /**
   * Select a session, or pass null to enter new-session mode. The only way to
   * change either value: it fires the optimistic UI switch and then tells the
   * backend, so no caller has to remember both halves.
   */
  readonly selectSessionAt: (sessionId: string | null, projectIdx: number) => void;
  /**
   * Move to a project without choosing a session in it — leaving the board on
   * a project that has none, for instance. Separate from `selectSessionAt`
   * because passing it a null session means "start a new one", which is a
   * different answer to a different question.
   */
  readonly selectProject: (projectIdx: number) => void;
}

export interface SessionSelectionOptions {
  readonly appState: any;
  /** Optimistically switch session UI (loading state, message cache, etc.) */
  readonly beginSessionSwitch: (targetSid: string, projectIdx?: number) => void;
}

// ── Hook ────────────────────────────────────────────────────

export function useSessionSelection(opts: SessionSelectionOptions): SessionSelection {
  const { appState, beginSessionSwitch } = opts;

  const [sessionId, setSessionId] = useState<string | null>(null);
  const [projectIndex, setProjectIndex] = useState(0);
  const [newSessionMode, setNewSessionMode] = useState(false);

  // What has already been pushed to the UI and to the backend, so a re-render
  // never repeats either. They are separate because the UI switch fires
  // immediately while the API call waits for `appState`.
  const syncedRef = useRef<{ sid: string | null; proj: number }>({ sid: null, proj: 0 });
  const apiSentRef = useRef<{ sid: string | null; proj: number }>({ sid: null, proj: 0 });

  const selectSessionAt = useCallback((next: string | null, projectIdx: number) => {
    setSessionId(next);
    setNewSessionMode(!next);
    setProjectIndex(projectIdx);
    // Choosing a session means "show me that conversation", so leave whatever
    // page we are on. Already on the chat root, this is a no-op rather than a
    // history entry.
    if (onPageRoute()) appNavigate("/");
  }, []);

  const selectProject = useCallback((projectIdx: number) => {
    setProjectIndex(projectIdx);
    if (onPageRoute()) appNavigate("/");
  }, []);

  // ── Push the selection outward ────────────────────────────

  const beginRef = useRef(beginSessionSwitch);
  beginRef.current = beginSessionSwitch;

  // `appState` is read through a ref so this effect runs when the *target*
  // changes, not on every SSE-driven state refresh — otherwise every server
  // update would re-issue selectSession().
  const appStateRef = useRef(appState);
  appStateRef.current = appState;

  // Only whether it has arrived, never its contents. On first load the session
  // is chosen before the first fetch resolves, so the effect has to run once
  // more when it does — but re-running on every subsequent refresh would
  // re-issue the API calls on each server event.
  const appStateReady = Boolean(appState);

  useEffect(() => {
    if (!sessionId) return;

    if (sessionId !== syncedRef.current.sid || projectIndex !== syncedRef.current.proj) {
      syncedRef.current = { sid: sessionId, proj: projectIndex };
      beginRef.current(sessionId, projectIndex);
    }

    const state = appStateRef.current;
    if (!state) return;
    if (sessionId === apiSentRef.current.sid && projectIndex === apiSentRef.current.proj) return;
    apiSentRef.current = { sid: sessionId, proj: projectIndex };
    void (async () => {
      try {
        if (projectIndex !== state.active_project) await switchProject(projectIndex);
        await selectSession(projectIndex, sessionId);
      } catch {
        // The session may be gone; useSessionRestore repairs a stale one.
      }
    })();
  }, [sessionId, projectIndex, appStateReady]);

  return { sessionId, projectIndex, newSessionMode, selectSessionAt, selectProject };
}
