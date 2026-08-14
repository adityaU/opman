/**
 * The actions that never cross a window boundary.
 *
 * Split from `reducer.ts` so that file stays about the workspace — which window
 * is active, moving a pane between windows, the chrome — and this one stays about
 * a single window's tree. Every case here answers the same question: what does
 * this one window look like afterwards.
 *
 * `emptyWindow` lives here rather than beside `emptyWorkspace` because closing
 * the last pane reseeds a window with a fresh one, so this file needs it and
 * `reducer.ts` re-exports it.
 */

import { uuid } from "../utils/uuid";
import { movePaneWithin } from "./move";
import { cyclePane, neighbour, paneByOrdinal } from "./nav";
import {
  amendWidget,
  jumpPaneHistory,
  openWidget,
  stepPaneHistory,
} from "./paneTarget";
import {
  equalize,
  findPane,
  newPane,
  onlyPane,
  paneCount,
  paneIds,
  removePane,
  resizeSplit,
  splitPane,
  swapPanes,
  swapWidgets,
} from "./tree";
import { asWindowId, type PaneId, type WidgetState, type WorkspaceWindow } from "./types";
import type { WorkspaceAction } from "./reducer";

export function emptyWindow(name: string, widget: WidgetState | null = null): WorkspaceWindow {
  const root = newPane(widget);
  return { id: asWindowId(uuid()), name, root, focusedPaneId: root.id, zoomedPaneId: null };
}

export function reduceWindow(window: WorkspaceWindow, action: WorkspaceAction): WorkspaceWindow {
  switch (action.type) {
    case "splitPane": {
      const created = newPane(action.widgetForPane ? null : action.widget ?? null);
      const pane = action.widgetForPane ? { ...created, widget: action.widgetForPane(created.id) } : created;
      const root = splitPane(window.root, action.pane, action.dir, pane, action.before);
      if (root === window.root) return window;
      // A split un-zooms: the point of splitting is to see both.
      return { ...window, root, focusedPaneId: created.id, zoomedPaneId: null };
    }

    case "closePane": {
      const root = removePane(window.root, action.pane);
      if (root === window.root) return window;
      // The last pane is emptied rather than removed; closing the window is a
      // separate, explicit action and should never happen by accident.
      if (root === null) return { ...emptyWindow(window.name), id: window.id };
      return refocus({ ...window, root, zoomedPaneId: null }, window.focusedPaneId);
    }

    case "closeOthers": {
      const root = onlyPane(window.root, action.pane);
      return { ...window, root, focusedPaneId: action.pane, zoomedPaneId: null };
    }

    case "openWidget": {
      const root = openWidget(window.root, action.pane, action.widget);
      return root === window.root ? window : { ...window, root, focusedPaneId: action.pane };
    }

    // No focus move: a write that only adds detail to what a pane already shows
    // is not the user pointing at that pane, and stealing focus for it would
    // pull the caret out of whatever they were typing in.
    case "amendWidget": {
      const root = amendWidget(window.root, action.pane, action.widget);
      return root === window.root ? window : { ...window, root };
    }

    case "historyStep": {
      const root = stepPaneHistory(window.root, action.pane, action.step, action.seq);
      return root === window.root ? window : { ...window, root, focusedPaneId: action.pane };
    }

    case "historyJump": {
      const root = jumpPaneHistory(window.root, action.pane, action.index, action.seq);
      return root === window.root ? window : { ...window, root, focusedPaneId: action.pane };
    }

    // Re-focusing the pane that already has focus must return the *same*
    // window. Every pane claims focus on `focusin`, and a window switch fires
    // one — so a new object here rebuilds the window on arrival and re-renders
    // the tree `WindowView`'s memo exists to leave alone.
    case "focusPane":
      if (action.pane === window.focusedPaneId) return window;
      return findPane(window.root, action.pane) ? { ...window, focusedPaneId: action.pane } : window;

    case "focusDirection": {
      const next = neighbour(window.root, window.focusedPaneId, action.dir);
      return next ? { ...window, focusedPaneId: next } : window;
    }

    case "focusOrdinal": {
      const next = paneByOrdinal(window.root, action.ordinal);
      return next ? { ...window, focusedPaneId: next } : window;
    }

    case "cycleFocus": {
      const next = cyclePane(window.root, window.focusedPaneId, action.step);
      return next ? { ...window, focusedPaneId: next } : window;
    }

    case "movePane": {
      const target = neighbour(window.root, window.focusedPaneId, action.dir);
      if (!target) return window;
      return { ...window, root: swapPanes(window.root, window.focusedPaneId, target) };
    }

    case "swapWidgets": {
      const root = swapWidgets(window.root, action.from, action.to);
      // Focus follows the widget the user just dropped, not the pane it left.
      return root === window.root ? window : { ...window, root, focusedPaneId: action.to };
    }

    /**
     * A pane dropped somewhere in its own window. An edge drop moves the frame
     * and un-zooms — the point of moving a pane is to see it next to the other
     * one — while a centre drop only trades widgets and leaves the zoom alone.
     */
    case "dropPane": {
      const root = movePaneWithin(window.root, action.pane, action.target, action.edge);
      if (root === window.root) return window;
      // Focus follows what the user just dropped: the pane itself when it moved,
      // and the pane the widget landed in when the two were traded.
      const focusedPaneId = action.edge === "center" ? action.target : action.pane;
      if (action.edge === "center") return { ...window, root, focusedPaneId };
      return { ...window, root, focusedPaneId, zoomedPaneId: null };
    }

    case "resize": {
      const root = resizeSplit(window.root, action.split, action.index, action.delta);
      return root === window.root ? window : { ...window, root };
    }

    case "equalize":
      return { ...window, root: equalize(window.root) };

    case "toggleZoom": {
      if (window.zoomedPaneId) return { ...window, zoomedPaneId: null };
      if (paneCount(window.root) < 2) return window;
      return { ...window, zoomedPaneId: window.focusedPaneId };
    }

    default:
      return window;
  }
}

/** Keep the focus on a live pane after a removal, preferring the nearest one. */
function refocus(window: WorkspaceWindow, previous: PaneId): WorkspaceWindow {
  if (findPane(window.root, previous)) return { ...window, focusedPaneId: previous };
  const remaining = paneIds(window.root);
  return { ...window, focusedPaneId: remaining[0] };
}
