/**
 * The workspace reducer.
 *
 * Every mutation the UI and the keymap can perform goes through this one total
 * function, so the invariants live in one place and the whole model is testable
 * without rendering anything. Actions name the user's intent (`splitPane`), not
 * the tree edit that implements it.
 */

import { uuid } from "../utils/uuid";
import type { DropEdge } from "./move";
import { emptyWindow, reduceWindow } from "./reducerWindow";
import { findPane, newPane, paneCount, splitPane } from "./tree";
import {
  asWindowId,
  DEFAULT_CHROME,
  type ChromeState,
  type Direction,
  type PaneId,
  type PaneNode,
  type SplitDir,
  type SplitId,
  type WidgetState,
  type Workspace,
  type WorkspaceWindow,
  type WindowId,
} from "./types";

export type ChromeLevel = keyof ChromeState;

export type WorkspaceAction =
  | { type: "splitPane"; pane: PaneId; dir: SplitDir; widget?: WidgetState | null; widgetForPane?: (pane: PaneId) => WidgetState; before?: boolean }
  | { type: "closePane"; pane: PaneId }
  | { type: "closeOthers"; pane: PaneId }
  /**
   * Point a pane somewhere. Recorded in the pane's trail.
   *
   * Split from `amendWidget` rather than carrying a `record` flag, so that every
   * caller has to say which of the two it is and a new one cannot default into
   * the wrong answer. The pair replaced a single `setWidget`, which was doing
   * both jobs and therefore recording a per-pane engine change as a place the
   * user had been.
   */
  | { type: "openWidget"; pane: PaneId; widget: WidgetState | null }
  /** Add detail to the place a pane is already on. Never recorded. */
  | { type: "amendWidget"; pane: PaneId; widget: WidgetState | null }
  /**
   * Walk a pane's trail. `seq` is minted by the caller — the reducer stays pure
   * — and re-arms the entry so a panel that has already handled it acts again.
   */
  | { type: "historyStep"; pane: PaneId; step: 1 | -1; seq: number }
  | { type: "historyJump"; pane: PaneId; index: number; seq: number }
  | { type: "focusPane"; pane: PaneId }
  | { type: "focusDirection"; dir: Direction }
  | { type: "focusOrdinal"; ordinal: number }
  | { type: "cycleFocus"; step: 1 | -1 }
  | { type: "movePane"; dir: Direction }
  | { type: "swapWidgets"; from: PaneId; to: PaneId }
  /**
   * A dropped pane. `edge` says which of the two drops it was — a centre drop
   * trades the two widgets, an edge drop re-seats the pane on that side of the
   * target and lets its old siblings spread into the space.
   */
  | { type: "dropPane"; pane: PaneId; target: PaneId; edge: DropEdge }
  | { type: "resize"; split: SplitId; index: number; delta: number }
  | { type: "equalize" }
  | { type: "toggleZoom" }
  | { type: "newWindow"; widget?: WidgetState | null; widgetForPane?: (pane: PaneId) => WidgetState }
  | { type: "closeWindow"; window: WindowId }
  | { type: "renameWindow"; window: WindowId; name: string }
  | { type: "activateWindow"; window: WindowId }
  /**
   * Move a window in the rail. `before` is the window it lands in front of, or
   * null for the end of the rail — an anchor rather than an index, because the
   * only thing the drag knows is which chip the pointer is over.
   */
  | { type: "reorderWindow"; window: WindowId; before: WindowId | null }
  | { type: "stepWindow"; step: 1 | -1 }
  | { type: "movePaneToWindow"; pane: PaneId; window: WindowId | "new" }
  | { type: "toggleChrome"; level: ChromeLevel }
  | { type: "toggleZen" }
  | { type: "replace"; workspace: Workspace };

// ── Seeds ───────────────────────────────────────────────

export { emptyWindow };

export function emptyWorkspace(): Workspace {
  const first = emptyWindow("1");
  return { windows: [first], activeWindowId: first.id, chrome: DEFAULT_CHROME };
}

// ── Reducer ─────────────────────────────────────────────

export function workspaceReducer(state: Workspace, action: WorkspaceAction): Workspace {
  switch (action.type) {
    case "replace":
      return action.workspace;

    case "toggleChrome":
      return { ...state, chrome: { ...state.chrome, [action.level]: !state.chrome[action.level] } };

    /**
     * Zen: the focused pane takes the whole shell and everything else stands
     * down — sidebar, window rail, pane headers, status bar.
     *
     * It reuses `zoomedPaneId` rather than introducing a second "one pane
     * fills the area" flag, because two of those would be two things to keep
     * agreeing with each other. Zen is therefore zoom plus silence, and
     * `toggleZoom` remains the quiet half on its own.
     *
     * No `paneCount < 2` guard, unlike zoom: with one pane there is nothing to
     * zoom past, but there is still a sidebar and a rail worth clearing away.
     */
    case "toggleZen": {
      const zen = !state.chrome.zen;
      return {
        ...state,
        chrome: { ...state.chrome, zen },
        windows: state.windows.map((window) =>
          window.id === state.activeWindowId
            ? { ...window, zoomedPaneId: zen ? window.focusedPaneId : null }
            : window,
        ),
      };
    }

    case "activateWindow":
      return state.windows.some((w) => w.id === action.window)
        ? { ...state, activeWindowId: action.window }
        : state;

    case "stepWindow": {
      const index = state.windows.findIndex((w) => w.id === state.activeWindowId);
      if (index < 0) return state;
      const next = (index + action.step + state.windows.length) % state.windows.length;
      return { ...state, activeWindowId: state.windows[next].id };
    }

    case "newWindow": {
      const created = emptyWindow(nextWindowName(state), action.widget ?? null);
      const root = action.widgetForPane && created.root.type === "leaf"
        ? { ...created.root, widget: action.widgetForPane(created.root.id) }
        : created.root;
      const window = root === created.root ? created : { ...created, root };
      return { ...state, windows: [...state.windows, window], activeWindowId: window.id };
    }

    case "reorderWindow":
      return reorderWindow(state, action.window, action.before);

    case "closeWindow":
      return closeWindow(state, action.window);

    case "renameWindow":
      return mapWindow(state, action.window, (w) => ({ ...w, name: action.name.trim() || w.name }));

    case "movePaneToWindow":
      return movePaneToWindow(state, action.pane, action.window);

    default: {
      const next = mapActive(state, (window) => reduceWindow(window, action));
      // Splitting, closing and focusing all clear `zoomedPaneId`. If that
      // happened while Zen was on, the chrome would stay hidden around a tree
      // that is no longer zoomed — Zen has to end with the zoom it rode in on.
      if (!next.chrome.zen) return next;
      const active = next.windows.find((window) => window.id === next.activeWindowId);
      if (active?.zoomedPaneId) return next;
      return { ...next, chrome: { ...next.chrome, zen: false } };
    }
  }
}

// ── Helpers ─────────────────────────────────────────────

function mapWindow(
  state: Workspace,
  id: WindowId,
  replace: (window: WorkspaceWindow) => WorkspaceWindow,
): Workspace {
  const windows = state.windows.map((w) => (w.id === id ? replace(w) : w));
  return windows.every((w, i) => w === state.windows[i]) ? state : { ...state, windows };
}

function mapActive(state: Workspace, replace: (w: WorkspaceWindow) => WorkspaceWindow): Workspace {
  return mapWindow(state, state.activeWindowId, replace);
}

function nextWindowName(state: Workspace): string {
  const used = new Set(state.windows.map((w) => w.name));
  for (let n = 1; n <= state.windows.length + 1; n += 1) {
    if (!used.has(String(n))) return String(n);
  }
  return String(state.windows.length + 1);
}

/**
 * Rail order is the `windows` array's order, so a reorder is a splice and
 * nothing else — the trees, the focus and the active window are untouched, and
 * the layout is persisted as a whole, so the new order outlives the tab.
 *
 * A drop that would not move the window returns the same state, so a click that
 * ends as a one-pixel drag never rebuilds the workspace.
 */
function reorderWindow(state: Workspace, moving: WindowId, before: WindowId | null): Workspace {
  if (before === moving) return state;
  const from = state.windows.findIndex((w) => w.id === moving);
  if (from < 0) return state;

  const rest = state.windows.filter((w) => w.id !== moving);
  const at = before === null ? rest.length : rest.findIndex((w) => w.id === before);
  if (at < 0 || at === from) return state;
  return { ...state, windows: [...rest.slice(0, at), state.windows[from], ...rest.slice(at)] };
}

function closeWindow(state: Workspace, id: WindowId): Workspace {
  if (state.windows.length <= 1) return state;
  const index = state.windows.findIndex((w) => w.id === id);
  if (index < 0) return state;
  const windows = state.windows.filter((w) => w.id !== id);
  const active =
    state.activeWindowId === id
      ? windows[Math.min(index, windows.length - 1)].id
      : state.activeWindowId;
  return { ...state, windows, activeWindowId: active };
}

/**
 * Detach a pane from the active window and land it in another, or in a new
 * one. The widget travels; the pane node itself is re-created, because the
 * destination tree owns its own ids.
 *
 * The trail travels with the widget, for the reason `swapWidgets` gives: a pane
 * moved to another window is the same job continued somewhere else, and its
 * history is part of that job rather than of the frame it left behind.
 */
function movePaneToWindow(state: Workspace, pane: PaneId, target: WindowId | "new"): Workspace {
  const source = state.windows.find((w) => w.id === state.activeWindowId);
  if (!source) return state;
  const moving = findPane(source.root, pane);
  if (!moving || paneCount(source.root) < 2) return state;

  const detached = reduceWindow(source, { type: "closePane", pane });
  const reseat = (): PaneNode => ({ ...newPane(moving.widget), history: moving.history });

  if (target === "new") {
    const seat = reseat();
    const created: WorkspaceWindow = {
      id: asWindowId(uuid()),
      name: nextWindowName(state),
      root: seat,
      focusedPaneId: seat.id,
      zoomedPaneId: null,
    };
    return {
      ...state,
      windows: [...state.windows.map((w) => (w.id === source.id ? detached : w)), created],
      activeWindowId: created.id,
    };
  }

  const windows = state.windows.map((w) => {
    if (w.id === source.id) return detached;
    if (w.id !== target) return w;
    const created = reseat();
    return { ...w, root: splitPane(w.root, w.focusedPaneId, "row", created), focusedPaneId: created.id };
  });
  return { ...state, windows, activeWindowId: target };
}
