import { useState, useCallback, useRef, useEffect } from "react";
import { selectSession, switchProject } from "../api";

// ── Types ───────────────────────────────────────────────────

export interface UrlSessionState {
  /** Active session ID derived from the URL — the single source of truth. */
  urlSessionId: string | null;
  /** Active project index derived from the URL — the single source of truth. */
  urlProjectIndex: number;
  /**
   * Navigate to a different session. This is the ONLY way to change the active session.
   * Updates the URL immediately (pushState), then fires beginSessionSwitch + API calls.
   */
  setUrlSession: (sessionId: string, projectIdx: number) => void;
}

export interface UrlSessionStateOptions {
  appState: any;
  /** Optimistically switch session UI (loading state, message cache, etc.) */
  beginSessionSwitch: (targetSid: string, projectIdx?: number) => void;
}

// ── Read session+project from URL ──────────────────────────

function readSessionFromUrl(): { sessionId: string | null; projectIdx: number } {
  const params = new URLSearchParams(window.location.search);
  return {
    sessionId: params.get("session") || null,
    projectIdx: params.has("project") ? Number(params.get("project")) : 0,
  };
}

// ── Hook ────────────────────────────────────────────────────

export function useUrlSessionState(opts: UrlSessionStateOptions): UrlSessionState {
  const { appState, beginSessionSwitch } = opts;

  // Read initial URL values
  const [initial] = useState(readSessionFromUrl);
  const [urlSessionId, setUrlSessionId] = useState<string | null>(initial.sessionId);
  const [urlProjectIndex, setUrlProjectIndex] = useState<number>(initial.projectIdx);

  // Track what we've already synced to avoid duplicate API calls.
  // Starts null so the initial URL values trigger the first sync effect.
  const syncedRef = useRef<{ sid: string | null; proj: number }>({
    sid: null,
    proj: 0,
  });

  // ── setUrlSession: update URL → state updates reactively ──

  const setUrlSession = useCallback((sessionId: string, projectIdx: number) => {
    setUrlSessionId(sessionId);
    setUrlProjectIndex(projectIdx);
  }, []);

  // ── React to session/project changes ──────────────────────
  // Fires beginSessionSwitch + API calls whenever the URL-derived values change.
  // Also handles initial URL restore: syncedRef starts at {null, 0} so the
  // first URL values trigger the effect once appState is available.

  const beginRef = useRef(beginSessionSwitch);
  beginRef.current = beginSessionSwitch;

  // Track appState in a ref — the effect should NOT re-run when appState changes,
  // only when the URL target changes. Reading appState via ref avoids spurious
  // selectSession() API calls on every SSE-driven refreshState() update.
  const appStateRef = useRef(appState);
  appStateRef.current = appState;

  // Track whether we've sent API calls for the current target (separate from UI sync).
  // This lets us fire beginSessionSwitch immediately but defer API calls until appState arrives.
  const apiSentRef = useRef<{ sid: string | null; proj: number }>({ sid: null, proj: 0 });

  useEffect(() => {
    if (!urlSessionId) return;

    const prev = syncedRef.current;
    const needsUiSync = urlSessionId !== prev.sid || urlProjectIndex !== prev.proj;
    const needsApi = urlSessionId !== apiSentRef.current.sid || urlProjectIndex !== apiSentRef.current.proj;

    // Fire beginSessionSwitch immediately (even before appState) for instant UI feedback
    if (needsUiSync) {
      syncedRef.current = { sid: urlSessionId, proj: urlProjectIndex };
      beginRef.current(urlSessionId, urlProjectIndex);
    }

    // Fire API calls — read appState from ref (no dep on appState changes)
    const currentAppState = appStateRef.current;
    if (needsApi && currentAppState) {
      apiSentRef.current = { sid: urlSessionId, proj: urlProjectIndex };
      (async () => {
        try {
          if (urlProjectIndex !== currentAppState.active_project) {
            await switchProject(urlProjectIndex);
          }
          await selectSession(urlProjectIndex, urlSessionId);
        } catch {
          // Silently ignore — URL might have a stale session
        }
      })();
    }
  }, [urlSessionId, urlProjectIndex]);

  // Handle deferred API call when appState arrives after the URL was already set.
  // This covers the initial page load where urlSessionId is set from URL params
  // before appState is fetched.  Without this, the selectSession() call would be
  // skipped and the server would never learn about the URL-selected session.
  const deferredApiSent = useRef(false);
  useEffect(() => {
    if (deferredApiSent.current || !appState || !urlSessionId) return;
    if (apiSentRef.current.sid === urlSessionId && apiSentRef.current.proj === urlProjectIndex) return;
    deferredApiSent.current = true;
    apiSentRef.current = { sid: urlSessionId, proj: urlProjectIndex };
    (async () => {
      try {
        if (urlProjectIndex !== appState.active_project) {
          await switchProject(urlProjectIndex);
        }
        await selectSession(urlProjectIndex, urlSessionId);
      } catch { /* ignore */ }
    })();
  }, [appState, urlSessionId, urlProjectIndex]);

  // ── Listen for popstate (browser back/forward) ────────────

  useEffect(() => {
    const handler = (e: PopStateEvent) => {
      // Ignore mobile overlay history entries
      const st = e.state as Record<string, unknown> | null;
      if (st && ("_modalLayer" in st || "_mobileOverlay" in st)) return;

      const { sessionId, projectIdx } = readSessionFromUrl();
      if (sessionId) {
        setUrlSessionId(sessionId);
        setUrlProjectIndex(projectIdx);
      }
    };
    window.addEventListener("popstate", handler);
    return () => window.removeEventListener("popstate", handler);
  }, []);

  return { urlSessionId, urlProjectIndex, setUrlSession };
}
