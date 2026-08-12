/**
 * The workspace reducer.
 *
 * Every mutation the UI and the keymap can perform goes through this one total
 * function, so the invariants live in one place and the whole model is testable
 * without rendering anything. Actions name the user's intent (`splitPane`), not
 * the tree edit that implements it.
 */

import { uuid } from "../utils/uuid";
import { cyclePane, neighbour, paneByOrdinal } from "./nav";
import {
  equalize,
  findPane,
  newPane,
  onlyPane,
  paneCount,
  paneIds,
  removePane,
  resizeSplit,
  setWidget,
  splitPane,
  swapPanes,
  swapWidgets,
} from "./tree";
import {
  asWindowId,
  DEFAULT_CHROME,
  type ChromeState,
  type Direction,
  type PaneId,
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
  | { type: "setWidget"; pane: PaneId; widget: WidgetState | null }
  | { type: "focusPane"; pane: PaneId }
  | { type: "focusDirection"; dir: Direction }
  | { type: "focusOrdinal"; ordinal: number }
  | { type: "cycleFocus"; step: 1 | -1 }
  | { type: "movePane"; dir: Direction }
  | { type: "swapWidgets"; from: PaneId; to: PaneId }
  | { type: "resize"; split: SplitId; index: number; delta: number }
  | { type: "equalize" }
  | { type: "toggleZoom" }
  | { type: "newWindow"; widget?: WidgetState | null; widgetForPane?: (pane: PaneId) => WidgetState }
  | { type: "closeWindow"; window: WindowId }
  | { type: "renameWindow"; window: WindowId; name: string }
  | { type: "activateWindow"; window: WindowId }
  | { type: "stepWindow"; step: 1 | -1 }
  | { type: "movePaneToWindow"; pane: PaneId; window: WindowId | "new" }
  | { type: "toggleChrome"; level: ChromeLevel }
  | { type: "toggleZen" }
  | { type: "replace"; workspace: Workspace };

// ── Seeds ───────────────────────────────────────────────

export function emptyWindow(name: string, widget: WidgetState | null = null): WorkspaceWindow {
  const root = newPane(widget);
  return { id: asWindowId(uuid()), name, root, focusedPaneId: root.id, zoomedPaneId: null };
}

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

// ── Window-local actions ────────────────────────────────

/**
 * Actions that never cross a window boundary. Split out so the top-level
 * switch stays about the workspace and this one stays about a single tree.
 */
function reduceWindow(window: WorkspaceWindow, action: WorkspaceAction): WorkspaceWindow {
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

    case "setWidget": {
      const root = setWidget(window.root, action.pane, action.widget);
      return root === window.root ? window : { ...window, root, focusedPaneId: action.pane };
    }

    case "focusPane":
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

/** Keep the focus on a live pane after a removal, preferring the nearest one. */
function refocus(window: WorkspaceWindow, previous: PaneId): WorkspaceWindow {
  if (findPane(window.root, previous)) return { ...window, focusedPaneId: previous };
  const remaining = paneIds(window.root);
  return { ...window, focusedPaneId: remaining[0] };
}

function nextWindowName(state: Workspace): string {
  const used = new Set(state.windows.map((w) => w.name));
  for (let n = 1; n <= state.windows.length + 1; n += 1) {
    if (!used.has(String(n))) return String(n);
  }
  return String(state.windows.length + 1);
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
 */
function movePaneToWindow(state: Workspace, pane: PaneId, target: WindowId | "new"): Workspace {
  const source = state.windows.find((w) => w.id === state.activeWindowId);
  if (!source) return state;
  const moving = findPane(source.root, pane);
  if (!moving || paneCount(source.root) < 2) return state;

  const detached = reduceWindow(source, { type: "closePane", pane });

  if (target === "new") {
    const created = emptyWindow(nextWindowName(state), moving.widget);
    return {
      ...state,
      windows: [...state.windows.map((w) => (w.id === source.id ? detached : w)), created],
      activeWindowId: created.id,
    };
  }

  const windows = state.windows.map((w) => {
    if (w.id === source.id) return detached;
    if (w.id !== target) return w;
    const created = newPane(moving.widget);
    return { ...w, root: splitPane(w.root, w.focusedPaneId, "row", created), focusedPaneId: created.id };
  });
  return { ...state, windows, activeWindowId: target };
}
