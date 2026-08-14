/**
 * Dragging a pane somewhere else: within its window, and out of it.
 *
 * The interesting cases are all about what the *other* panes do — a move is a
 * lift and a re-seat, so the siblings left behind have to close the gap and the
 * tree has to stay legal on both sides of the operation.
 */
import { describe, it, expect } from "vitest";
import { edgeFromPointer, movePaneWithin, type DropEdge } from "../workspace/move";
import { emptyWorkspace, workspaceReducer } from "../workspace/reducer";
import { newPane, paneCount, paneIds, panes, splitPane } from "../workspace/tree";
import type { Node, PaneId, WidgetState, Workspace } from "../workspace/types";

const chat = (projectPath: string): WidgetState => ({
  kind: "chat",
  projectPath,
  sessionId: null,
  engine: null,
});

function legal(node: Node): void {
  if (node.type === "leaf") return;
  expect(node.children).toHaveLength(node.sizes.length);
  expect(node.children.length).toBeGreaterThanOrEqual(2);
  expect(node.sizes.reduce((sum, size) => sum + size, 0)).toBeCloseTo(1, 10);
  node.children.forEach(legal);
}

/** A row of three panes, each holding a differently-named chat. */
function threeInARow(): { root: Node; ids: readonly PaneId[] } {
  const first = newPane(chat("a"));
  const second = newPane(chat("b"));
  const third = newPane(chat("c"));
  const withSecond = splitPane(first, first.id, "row", second);
  const root = splitPane(withSecond, second.id, "row", third);
  return { root, ids: [first.id, second.id, third.id] };
}

const projectsOf = (root: Node): (string | null)[] =>
  panes(root).map((pane) => pane.widget?.projectPath ?? null);

describe("movePaneWithin", () => {
  it("re-orders a row without adding a level to the tree", () => {
    const { root, ids } = threeInARow();
    const moved = movePaneWithin(root, ids[2], ids[0], "left");

    expect(projectsOf(moved)).toEqual(["c", "a", "b"]);
    expect(paneCount(moved)).toBe(3);
    if (moved.type !== "split") throw new Error("expected a split");
    expect(moved.children.every((child) => child.type === "leaf")).toBe(true);
    legal(moved);
  });

  it("keeps the pane's id, so its shell and its focus scope survive the move", () => {
    const { root, ids } = threeInARow();
    const moved = movePaneWithin(root, ids[0], ids[2], "right");
    expect(new Set(paneIds(moved))).toEqual(new Set(ids));
  });

  it("collapses the split a departing pane leaves behind", () => {
    const { root, ids } = threeInARow();
    // Two of the three end up in a column, so the row is down to one child.
    const once = movePaneWithin(root, ids[0], ids[1], "bottom");
    const twice = movePaneWithin(once, ids[2], ids[1], "bottom");

    expect(paneCount(twice)).toBe(3);
    expect(twice.type).toBe("split");
    if (twice.type !== "split") return;
    expect(twice.dir).toBe("col");
    expect(twice.children).toHaveLength(3);
    legal(twice);
  });

  it("splits the target across the other axis", () => {
    const { root, ids } = threeInARow();
    const moved = movePaneWithin(root, ids[0], ids[2], "bottom");
    expect(projectsOf(moved)).toEqual(["b", "c", "a"]);
    legal(moved);
  });

  it("trades widgets for a centre drop and leaves the frames alone", () => {
    const { root, ids } = threeInARow();
    const moved = movePaneWithin(root, ids[0], ids[2], "center");
    expect(projectsOf(moved)).toEqual(["c", "b", "a"]);
    expect(paneIds(moved)).toEqual(paneIds(root));
  });

  it("is a no-op onto itself, onto a stranger, and out of a lone pane", () => {
    const { root, ids } = threeInARow();
    const stranger = newPane(chat("z")).id;
    expect(movePaneWithin(root, ids[0], ids[0], "left")).toBe(root);
    expect(movePaneWithin(root, ids[0], stranger, "left")).toBe(root);

    const alone = newPane(chat("a"));
    expect(movePaneWithin(alone, alone.id, alone.id, "right")).toBe(alone);
  });
});

describe("edgeFromPointer", () => {
  const box = new DOMRect(0, 0, 1000, 500);
  const at = (x: number, y: number): DropEdge => edgeFromPointer(box, x, y);

  it("reads the four sides and the middle", () => {
    expect(at(500, 250)).toBe("center");
    expect(at(20, 250)).toBe("left");
    expect(at(980, 250)).toBe("right");
    expect(at(500, 10)).toBe("top");
    expect(at(500, 490)).toBe("bottom");
  });

  it("gives a corner to the nearer side rather than always to the horizontal", () => {
    // 5% in from the left, 2% down: vertically the closer edge.
    expect(at(50, 10)).toBe("top");
    expect(at(10, 50)).toBe("left");
  });
});

describe("dropPane", () => {
  const seeded = (): Workspace => {
    const base = emptyWorkspace();
    const window = base.windows[0];
    return workspaceReducer(base, {
      type: "splitPane",
      pane: window.focusedPaneId,
      dir: "row",
      widget: chat("b"),
    });
  };

  it("moves the pane and follows it with the focus", () => {
    const state = seeded();
    const [left, right] = paneIds(state.windows[0].root);
    const next = workspaceReducer(state, {
      type: "dropPane",
      pane: right,
      target: left,
      edge: "top",
    });

    const window = next.windows[0];
    expect(window.focusedPaneId).toBe(right);
    expect(window.root.type).toBe("split");
    if (window.root.type !== "split") return;
    expect(window.root.dir).toBe("col");
    expect(paneIds(window.root)).toEqual([right, left]);
  });

  it("un-zooms an edge drop, because the point of the move is to see both", () => {
    const zoomed = workspaceReducer(seeded(), { type: "toggleZoom" });
    expect(zoomed.windows[0].zoomedPaneId).not.toBeNull();
    const [left, right] = paneIds(zoomed.windows[0].root);

    const next = workspaceReducer(zoomed, {
      type: "dropPane",
      pane: right,
      target: left,
      edge: "left",
    });
    expect(next.windows[0].zoomedPaneId).toBeNull();
  });

  it("focuses the receiving pane on a centre drop", () => {
    const state = seeded();
    const [left, right] = paneIds(state.windows[0].root);
    const next = workspaceReducer(state, {
      type: "dropPane",
      pane: right,
      target: left,
      edge: "center",
    });
    expect(next.windows[0].focusedPaneId).toBe(left);
    expect(paneIds(next.windows[0].root)).toEqual([left, right]);
  });

  it("leaves the workspace identical when nothing can move", () => {
    const state = seeded();
    const [left] = paneIds(state.windows[0].root);
    expect(
      workspaceReducer(state, { type: "dropPane", pane: left, target: left, edge: "left" }),
    ).toBe(state);
  });
});

describe("movePaneToWindow, as a drop", () => {
  it("lands the widget in the target window and activates it", () => {
    const base = workspaceReducer(emptyWorkspace(), { type: "newWindow", widget: chat("b") });
    const first = base.windows[0];
    const withTwo = workspaceReducer(
      { ...base, activeWindowId: first.id },
      { type: "splitPane", pane: first.focusedPaneId, dir: "row", widget: chat("c") },
    );
    const source = withTwo.windows[0];
    const moving = paneIds(source.root)[1];

    const next = workspaceReducer(withTwo, {
      type: "movePaneToWindow",
      pane: moving,
      window: withTwo.windows[1].id,
    });

    expect(paneCount(next.windows[0].root)).toBe(1);
    expect(projectsOf(next.windows[1].root)).toContain("c");
    expect(next.activeWindowId).toBe(next.windows[1].id);
  });

  it("opens a new window for a drop on the plus", () => {
    const base = emptyWorkspace();
    const withTwo = workspaceReducer(base, {
      type: "splitPane",
      pane: base.windows[0].focusedPaneId,
      dir: "row",
      widget: chat("b"),
    });
    const moving = paneIds(withTwo.windows[0].root)[1];

    const next = workspaceReducer(withTwo, {
      type: "movePaneToWindow",
      pane: moving,
      window: "new",
    });
    expect(next.windows).toHaveLength(2);
    expect(projectsOf(next.windows[1].root)).toEqual(["b"]);
    expect(paneCount(next.windows[0].root)).toBe(1);
  });
});
