import { useMemo } from "react";
import { panes } from "./tree";
import type { TargetSlot } from "./target/useTargeting";
import type { PaneId, PaneNode, Workspace, WorkspaceWindow, WindowId } from "./types";

/**
 * Windows with something working inside: a terminal running a command, or a
 * chat whose session is busy.
 */
export function useBusyWindows(
  windows: readonly WorkspaceWindow[],
  busySessions: ReadonlySet<string>,
  busyTerminals: ReadonlySet<PaneId>,
): ReadonlySet<WindowId> {
  return useMemo(() => {
    const busy = new Set<WindowId>();
    for (const window of windows) {
      const anyBusy = panes(window.root).some((pane) => {
        if (busyTerminals.has(pane.id)) return true;
        if (pane.widget?.kind !== "chat" || !pane.widget.sessionId) return false;
        return busySessions.has(pane.widget.sessionId);
      });
      if (anyBusy) busy.add(window.id);
    }
    return busy;
  }, [busySessions, busyTerminals, windows]);
}

/** The target overlay's numbered slots, and the windows a dragged pane can go to. */
export function useTargetSlots(
  state: Workspace,
  paneList: readonly PaneNode[],
  focusedPaneId: PaneId,
) {
  /**
   * The windows a dragged pane can be sent to: every one but the one it is in,
   * which is what the pane slots themselves already answer for.
   */
  const otherWindows = useMemo(
    () =>
      state.windows
        .filter((window) => window.id !== state.activeWindowId)
        .map((window) => ({ id: window.id, name: window.name })),
    [state.activeWindowId, state.windows],
  );

  const slots: TargetSlot[] = useMemo(
    () =>
      paneList.map((pane, index) => ({
        paneId: pane.id,
        ordinal: index + 1,
        focused: pane.id === focusedPaneId,
      })),
    [focusedPaneId, paneList],
  );

  return { otherWindows, slots };
}
