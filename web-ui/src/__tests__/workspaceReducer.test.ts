import { describe, it, expect } from "vitest";
import {
  emptyWorkspace,
  workspaceReducer,
  type WorkspaceAction,
} from "../workspace/reducer";
import { paneIds, paneCount } from "../workspace/tree";
import type { PaneId, WidgetState, Workspace, WindowId } from "../workspace/types";

const chat = (projectPath: string): WidgetState => ({ kind: "chat", projectPath, sessionId: null, engine: null });
const git = (projectPath: string): WidgetState => ({ kind: "git", projectPath });

const run = (state: Workspace, ...actions: WorkspaceAction[]): Workspace =>
  actions.reduce(workspaceReducer, state);

const active = (state: Workspace) => {
  const window = state.windows.find((w) => w.id === state.activeWindowId);
  if (!window) throw new Error("no active window");
  return window;
};

const firstPane = (state: Workspace): PaneId => paneIds(active(state).root)[0];

describe("seed", () => {
  it("starts with one window holding one empty pane", () => {
    const state = emptyWorkspace();
    expect(state.windows).toHaveLength(1);
    expect(paneCount(active(state).root)).toBe(1);
    expect(active(state).root).toMatchObject({ type: "leaf", widget: null });
    expect(active(state).focusedPaneId).toBe(firstPane(state));
  });
});

describe("panes", () => {
  it("focuses the pane a split created, so the next action lands in it", () => {
    const state = emptyWorkspace();
    const next = run(state, { type: "splitPane", pane: firstPane(state), dir: "row" });
    expect(paneCount(active(next).root)).toBe(2);
    expect(active(next).focusedPaneId).toBe(paneIds(active(next).root)[1]);
  });

  it("empties the last pane instead of leaving a window with none", () => {
    const state = run(emptyWorkspace(), {
      type: "setWidget",
      pane: firstPane(emptyWorkspace()),
      widget: chat("/a"),
    });
    const only = firstPane(state);
    const next = workspaceReducer(state, { type: "closePane", pane: only });
    expect(next.windows).toHaveLength(1);
    expect(paneCount(active(next).root)).toBe(1);
    expect(active(next).root).toMatchObject({ widget: null });
  });

  it("moves focus off a closed pane onto a live one", () => {
    let state = emptyWorkspace();
    state = run(state, { type: "splitPane", pane: firstPane(state), dir: "row" });
    const [, second] = paneIds(active(state).root);
    const next = workspaceReducer(state, { type: "closePane", pane: second });
    expect(paneIds(active(next).root)).toContain(active(next).focusedPaneId);
  });

  it("keeps only the named pane on closeOthers", () => {
    let state = emptyWorkspace();
    state = run(
      state,
      { type: "splitPane", pane: firstPane(state), dir: "row" },
      { type: "splitPane", pane: paneIds(active(state).root)[0], dir: "col" },
    );
    const keep = active(state).focusedPaneId;
    const next = workspaceReducer(state, { type: "closeOthers", pane: keep });
    expect(paneCount(active(next).root)).toBe(1);
    expect(active(next).focusedPaneId).toBe(keep);
  });
});

describe("zoom", () => {
  it("refuses to zoom a window that has nothing to hide", () => {
    const next = workspaceReducer(emptyWorkspace(), { type: "toggleZoom" });
    expect(active(next).zoomedPaneId).toBeNull();
  });

  it("zooms the focused pane and toggles back off", () => {
    let state = emptyWorkspace();
    state = run(state, { type: "splitPane", pane: firstPane(state), dir: "row" });
    const zoomed = workspaceReducer(state, { type: "toggleZoom" });
    expect(zoomed.windows[0].zoomedPaneId).toBe(active(state).focusedPaneId);
    expect(workspaceReducer(zoomed, { type: "toggleZoom" }).windows[0].zoomedPaneId).toBeNull();
  });

  it("un-zooms on a split, because splitting means wanting to see both", () => {
    let state = emptyWorkspace();
    state = run(state, { type: "splitPane", pane: firstPane(state), dir: "row" });
    state = workspaceReducer(state, { type: "toggleZoom" });
    const split = workspaceReducer(state, {
      type: "splitPane",
      pane: active(state).focusedPaneId,
      dir: "col",
    });
    expect(active(split).zoomedPaneId).toBeNull();
  });
});

describe("windows", () => {
  it("names new windows with the lowest free number", () => {
    const state = run(emptyWorkspace(), { type: "newWindow" }, { type: "newWindow" });
    expect(state.windows.map((w) => w.name)).toEqual(["1", "2", "3"]);
  });

  it("activates the window it just created", () => {
    const state = workspaceReducer(emptyWorkspace(), { type: "newWindow" });
    expect(state.activeWindowId).toBe(state.windows[1].id);
  });

  it("refuses to close the only window", () => {
    const state = emptyWorkspace();
    expect(workspaceReducer(state, { type: "closeWindow", window: state.windows[0].id })).toBe(state);
  });

  it("moves the active marker to a neighbour when the active window closes", () => {
    const state = run(emptyWorkspace(), { type: "newWindow" });
    const next = workspaceReducer(state, { type: "closeWindow", window: state.activeWindowId });
    expect(next.windows).toHaveLength(1);
    expect(next.activeWindowId).toBe(next.windows[0].id);
  });

  it("steps between windows and wraps", () => {
    let state = run(emptyWorkspace(), { type: "newWindow" });
    state = workspaceReducer(state, { type: "stepWindow", step: 1 });
    expect(state.activeWindowId).toBe(state.windows[0].id);
    state = workspaceReducer(state, { type: "stepWindow", step: -1 });
    expect(state.activeWindowId).toBe(state.windows[1].id);
  });

  it("ignores a rename to blank rather than losing the label", () => {
    const state = emptyWorkspace();
    const next = workspaceReducer(state, {
      type: "renameWindow",
      window: state.activeWindowId,
      name: "   ",
    });
    expect(active(next).name).toBe("1");
  });

  it("ignores activating a window that is not there", () => {
    const state = emptyWorkspace();
    expect(workspaceReducer(state, { type: "activateWindow", window: "ghost" as WindowId })).toBe(state);
  });
});

describe("movePaneToWindow", () => {
  it("carries the widget into a new window and leaves the source intact", () => {
    let state = emptyWorkspace();
    state = run(state, { type: "splitPane", pane: firstPane(state), dir: "row", widget: git("/b") });
    const moving = active(state).focusedPaneId;

    const next = workspaceReducer(state, { type: "movePaneToWindow", pane: moving, window: "new" });
    expect(next.windows).toHaveLength(2);
    expect(paneCount(next.windows[0].root)).toBe(1);
    expect(active(next).root).toMatchObject({ widget: git("/b") });
  });

  it("refuses to move a window's only pane, which would just empty it", () => {
    const state = emptyWorkspace();
    expect(
      workspaceReducer(state, { type: "movePaneToWindow", pane: firstPane(state), window: "new" }),
    ).toBe(state);
  });
});

describe("chrome", () => {
  // Pane headers are no longer chrome state: they are peeked on a chord and
  // withdraw on a timer, so there is nothing here to toggle for them.
  it("toggles each level independently", () => {
    let state = emptyWorkspace();
    expect(state.chrome).toEqual({ rail: true, zen: false });
    state = run(state, { type: "toggleChrome", level: "rail" });
    expect(state.chrome).toEqual({ rail: false, zen: false });
  });
});

describe("focus", () => {
  it("addresses panes by ordinal, which is what mod+1..9 and the overlay use", () => {
    let state = emptyWorkspace();
    state = run(state, { type: "splitPane", pane: firstPane(state), dir: "row" });
    const next = workspaceReducer(state, { type: "focusOrdinal", ordinal: 1 });
    expect(active(next).focusedPaneId).toBe(paneIds(active(next).root)[0]);
  });

  it("holds still at the edge instead of wrapping on a directional move", () => {
    let state = emptyWorkspace();
    state = run(
      state,
      { type: "splitPane", pane: firstPane(state), dir: "row" },
      { type: "focusOrdinal", ordinal: 1 },
    );
    const next = workspaceReducer(state, { type: "focusDirection", dir: "left" });
    expect(active(next).focusedPaneId).toBe(active(state).focusedPaneId);
  });

  it("ignores focusing a pane that is not in the tree", () => {
    const state = emptyWorkspace();
    expect(workspaceReducer(state, { type: "focusPane", pane: "ghost" as PaneId })).toBe(state);
  });
});

describe("swapWidgets", () => {
  it("swaps two panes' widgets and focuses the drop target", () => {
    let state = emptyWorkspace();
    const first = firstPane(state);
    state = run(state, { type: "setWidget", pane: first, widget: chat("/a") });
    state = run(state, { type: "splitPane", pane: first, dir: "row", widget: git("/b") });
    const [left, right] = paneIds(active(state).root);

    const next = run(state, { type: "swapWidgets", from: left, to: right });
    const [a, b] = paneIds(active(next).root);

    expect(a).toBe(left);
    expect(b).toBe(right);
    expect(active(next).focusedPaneId).toBe(right);
  });

  it("leaves the workspace untouched when the swap changes nothing", () => {
    const state = emptyWorkspace();
    const only = firstPane(state);
    expect(run(state, { type: "swapWidgets", from: only, to: only })).toBe(state);
  });
});

describe("zen", () => {
  it("zooms the focused pane and reports itself on", () => {
    let state = emptyWorkspace();
    const first = firstPane(state);
    state = run(state, { type: "splitPane", pane: first, dir: "row" });
    const focused = active(state).focusedPaneId;

    state = run(state, { type: "toggleZen" });

    expect(state.chrome.zen).toBe(true);
    expect(active(state).zoomedPaneId).toBe(focused);
  });

  it("works with a single pane, unlike zoom", () => {
    // Nothing to zoom past, but there is still a sidebar and a rail to clear.
    const state = run(emptyWorkspace(), { type: "toggleZen" });
    expect(state.chrome.zen).toBe(true);
    expect(active(state).zoomedPaneId).toBe(active(state).focusedPaneId);
  });

  it("releases the zoom when switched off", () => {
    let state = run(emptyWorkspace(), { type: "toggleZen" });
    state = run(state, { type: "toggleZen" });
    expect(state.chrome.zen).toBe(false);
    expect(active(state).zoomedPaneId).toBeNull();
  });

  it("ends when the zoom it rides on is cleared by a split", () => {
    // Splitting drops zoomedPaneId; zen must not be left holding the chrome
    // hidden around a tree that is no longer zoomed.
    let state = run(emptyWorkspace(), { type: "toggleZen" });
    state = run(state, { type: "splitPane", pane: active(state).focusedPaneId, dir: "row" });

    expect(active(state).zoomedPaneId).toBeNull();
    expect(state.chrome.zen).toBe(false);
  });
});
