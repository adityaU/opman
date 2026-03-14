import { useEffect, useRef, useCallback } from "react";

// ── URL search-param keys ───────────────────────────────────────────

const P_SESSION = "session";
const P_PROJECT = "project";
const P_SIDEBAR = "sidebar";
const P_TERMINAL = "terminal";
const P_EDITOR = "editor";
const P_GIT = "git";

// ── Types ───────────────────────────────────────────────────────────

/** Panel visibility state that we sync with the URL. */
export interface UrlPanelState {
  sidebar: boolean;
  terminal: boolean;
  editor: boolean;
  git: boolean;
}

/** Everything we persist in the URL. */
export interface UrlState {
  sessionId: string | null;
  projectIdx: number | null;
  panels: UrlPanelState;
}

// ── Read URL params ─────────────────────────────────────────────────

function boolParam(params: URLSearchParams, key: string, fallback: boolean): boolean {
  const v = params.get(key);
  if (v === null) return fallback;
  return v === "1" || v === "true";
}

/** Parse the current URL search params into a UrlState. */
export function readUrlState(): UrlState {
  const params = new URLSearchParams(window.location.search);
  return {
    sessionId: params.get(P_SESSION) || null,
    projectIdx: params.has(P_PROJECT) ? Number(params.get(P_PROJECT)) : null,
    panels: {
      sidebar: boolParam(params, P_SIDEBAR, true), // default open
      terminal: boolParam(params, P_TERMINAL, false),
      editor: boolParam(params, P_EDITOR, false),
      git: boolParam(params, P_GIT, false),
    },
  };
}

// ── Write URL params ────────────────────────────────────────────────

function buildSearchString(state: UrlState): string {
  const params = new URLSearchParams();

  if (state.sessionId) params.set(P_SESSION, state.sessionId);
  if (state.projectIdx !== null && state.projectIdx !== 0) {
    params.set(P_PROJECT, String(state.projectIdx));
  }

  // Only write non-default panel states to keep URLs short
  if (!state.panels.sidebar) params.set(P_SIDEBAR, "0");
  if (state.panels.terminal) params.set(P_TERMINAL, "1");
  if (state.panels.editor) params.set(P_EDITOR, "1");
  if (state.panels.git) params.set(P_GIT, "1");

  const s = params.toString();
  return s ? `?${s}` : window.location.pathname;
}

// ── Hook ────────────────────────────────────────────────────────────

interface UseUrlStateOptions {
  /** Current active session ID from app state */
  sessionId: string | null;
  /** Current active project index */
  projectIdx: number;
  /** Panel visibility */
  panels: UrlPanelState;
  /** Called when user navigates back/forward and state should be restored */
  onPopState: (state: UrlState) => void;
}

/**
 * Syncs application navigation state to the URL search params.
 *
 * - Session changes use `pushState` (creates history entries for back/forward).
 * - Panel toggles use `replaceState` (no history clutter).
 * - Listens to `popstate` for browser back/forward navigation.
 */
export function useUrlState({
  sessionId,
  projectIdx,
  panels,
  onPopState,
}: UseUrlStateOptions) {
  const prevStateRef = useRef<UrlState | null>(null);
  const isPopStateRef = useRef(false);

  // Sync current state → URL
  useEffect(() => {
    // Skip URL update while handling a popstate event
    if (isPopStateRef.current) {
      isPopStateRef.current = false;
      return;
    }

    const current: UrlState = { sessionId, projectIdx, panels };
    const prev = prevStateRef.current;
    prevStateRef.current = current;

    const url = buildSearchString(current);

    // Session change → pushState (new history entry)
    // Panel toggle → replaceState (no history clutter)
    if (prev && prev.sessionId !== current.sessionId) {
      window.history.pushState(current, "", url);
    } else {
      window.history.replaceState(current, "", url);
    }
  }, [sessionId, projectIdx, panels.sidebar, panels.terminal, panels.editor, panels.git]);

  // Listen for popstate (browser back/forward)
  const onPopStateRef = useRef(onPopState);
  onPopStateRef.current = onPopState;

  useEffect(() => {
    const handler = (e: PopStateEvent) => {
      // Ignore history entries pushed by modal/drawer back-gesture support.
      // Those entries carry a `_modalLayer` or `_mobileOverlay` sentinel and
      // are managed by useModalState / useMobileState respectively.
      const st = e.state as Record<string, unknown> | null;
      if (st && ("_modalLayer" in st || "_mobileOverlay" in st)) return;

      isPopStateRef.current = true;
      const restored = readUrlState();
      prevStateRef.current = restored;
      onPopStateRef.current(restored);
    };
    window.addEventListener("popstate", handler);
    return () => window.removeEventListener("popstate", handler);
  }, []);

  // Set initial history state on mount
  useEffect(() => {
    const initial: UrlState = { sessionId, projectIdx, panels };
    window.history.replaceState(initial, "", buildSearchString(initial));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
