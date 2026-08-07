/**
 * The workspace hook: reducer, persistence and the selectors the shell needs.
 *
 * Writes are debounced because a resize drag dispatches on every pointer move,
 * and serialising the whole tree at 60Hz is the one way a pure-JSON layout can
 * become a performance problem.
 */

import { useEffect, useMemo, useReducer, useRef } from "react";
import { loadWorkspace, saveWorkspace } from "./persistence";
import { emptyWorkspace, workspaceReducer, type WorkspaceAction } from "./reducer";
import { findPane, panes } from "./tree";
import type { PaneNode, Workspace, WorkspaceWindow } from "./types";

const SAVE_DEBOUNCE_MS = 300;

export interface WorkspaceApi {
  readonly state: Workspace;
  readonly window: WorkspaceWindow;
  /** Panes of the active window in reading order — the number overlay's order. */
  readonly panes: readonly PaneNode[];
  readonly focusedPane: PaneNode | null;
  readonly dispatch: (action: WorkspaceAction) => void;
}

export function useWorkspace(enabled: boolean): WorkspaceApi {
  // Mobile never reads the workspace, and parsing it there would be work done
  // for a surface that will not render.
  const [state, dispatch] = useReducer(
    workspaceReducer,
    enabled,
    (on): Workspace => (on ? loadWorkspace() : emptyWorkspace()),
  );

  usePersist(state, enabled);

  const window =
    state.windows.find((candidate) => candidate.id === state.activeWindowId) ?? state.windows[0];

  const paneList = useMemo(() => panes(window.root), [window.root]);
  const focusedPane = useMemo(
    () => findPane(window.root, window.focusedPaneId),
    [window.root, window.focusedPaneId],
  );

  return useMemo(
    () => ({ state, window, panes: paneList, focusedPane, dispatch }),
    [state, window, paneList, focusedPane],
  );
}

/** Debounced write-behind, with a final flush so a fast reload keeps the edit. */
function usePersist(state: Workspace, enabled: boolean): void {
  const latest = useRef(state);
  latest.current = state;
  const first = useRef(true);

  useEffect(() => {
    if (!enabled) return;
    // The initial state came *from* storage; writing it straight back would
    // only risk overwriting a good value with a repaired one on every mount.
    if (first.current) {
      first.current = false;
      return;
    }
    const timer = setTimeout(() => saveWorkspace(latest.current), SAVE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [state, enabled]);

  useEffect(() => {
    if (!enabled) return;
    const flush = () => saveWorkspace(latest.current);
    // `pagehide` fires where `beforeunload` does not on mobile Safari, and both
    // fire before the tab is frozen; a double write is cheap.
    globalThis.addEventListener("pagehide", flush);
    globalThis.addEventListener("beforeunload", flush);
    return () => {
      globalThis.removeEventListener("pagehide", flush);
      globalThis.removeEventListener("beforeunload", flush);
      flush();
    };
  }, [enabled]);
}
