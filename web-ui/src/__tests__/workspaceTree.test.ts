/**
 * Tree invariants. Every assertion here is something the renderer and the
 * keymap both assume and neither checks.
 */
import { describe, it, expect } from "vitest";
import {
  equalize,
  evenSizes,
  findPane,
  newPane,
  normalize,
  paneCount,
  paneIds,
  panes,
  removePane,
  resizeSplit,
  setWidget,
  splitPane,
  swapPanes,
  swapWidgets,
} from "../workspace/tree";
import { neighbour, cyclePane, paneByOrdinal, ordinalOfPane } from "../workspace/nav";
import { MIN_PANE_FRACTION, type Node, type PaneId, type SplitNode, type WidgetState } from "../workspace/types";

const chat = (projectPath: string): WidgetState => ({ kind: "chat", projectPath, sessionId: null, engine: null });

function sumsToOne(node: Node): void {
  if (node.type === "leaf") return;
  const total = node.sizes.reduce((sum, size) => sum + size, 0);
  expect(total).toBeCloseTo(1, 10);
  expect(node.sizes).toHaveLength(node.children.length);
  expect(node.children.length).toBeGreaterThanOrEqual(2);
  node.children.forEach(sumsToOne);
}

describe("evenSizes / normalize", () => {
  it("always sums to exactly 1", () => {
    for (const n of [1, 2, 3, 7, 11]) {
      expect(evenSizes(n).reduce((a, b) => a + b, 0)).toBeCloseTo(1, 12);
    }
  });

  it("rescales drifted sizes rather than trusting them", () => {
    expect(normalize([2, 2]).reduce((a, b) => a + b, 0)).toBeCloseTo(1, 12);
  });

  it("falls back to even shares when the sizes are degenerate", () => {
    expect(normalize([0, 0])).toEqual(evenSizes(2));
  });
});

describe("splitPane", () => {
  it("turns a lone pane into a two-child split", () => {
    const root = newPane();
    const next = splitPane(root, root.id, "row", newPane());
    expect(next.type).toBe("split");
    expect(paneCount(next)).toBe(2);
    sumsToOne(next);
  });

  it("appends to the parent instead of nesting when the direction matches", () => {
    let root: Node = newPane();
    const first = root.id;
    root = splitPane(root, first, "row", newPane());
    const second = paneIds(root)[1];
    root = splitPane(root, second, "row", newPane());

    expect(root.type).toBe("split");
    expect((root as SplitNode).children).toHaveLength(3);
    expect((root as SplitNode).children.every((c) => c.type === "leaf")).toBe(true);
    sumsToOne(root);
  });

  it("nests when the direction differs", () => {
    const leaf = newPane();
    const row = splitPane(leaf, leaf.id, "row", newPane());
    const nested = splitPane(row, paneIds(row)[0], "col", newPane());
    expect((nested as SplitNode).dir).toBe("row");
    expect((nested as SplitNode).children[0].type).toBe("split");
    sumsToOne(nested);
  });

  it("halves only the pane it split, leaving siblings alone", () => {
    let root: Node = newPane();
    root = splitPane(root, root.id, "row", newPane());
    root = splitPane(root, paneIds(root)[0], "row", newPane());
    const sizes = (root as SplitNode).sizes;
    expect(sizes[0]).toBeCloseTo(0.25, 10);
    expect(sizes[1]).toBeCloseTo(0.25, 10);
    expect(sizes[2]).toBeCloseTo(0.5, 10);
  });

  it("places the new pane before the target when asked", () => {
    const leaf = newPane();
    const inserted = newPane();
    const root = splitPane(leaf, leaf.id, "row", inserted, true);
    expect(paneIds(root)[0]).toBe(inserted.id);
  });

  it("is a no-op for an unknown pane", () => {
    const root = newPane();
    expect(splitPane(root, "nope" as PaneId, "row", newPane())).toBe(root);
  });
});

describe("removePane", () => {
  it("collapses a split back into its only remaining child", () => {
    const leaf = newPane();
    const root = splitPane(leaf, leaf.id, "row", newPane());
    const removed = removePane(root, paneIds(root)[1]);
    expect(removed?.type).toBe("leaf");
    expect((removed as Node & { id: PaneId }).id).toBe(leaf.id);
  });

  it("keeps the remaining siblings summing to 1", () => {
    let root: Node = newPane();
    root = splitPane(root, root.id, "row", newPane());
    root = splitPane(root, paneIds(root)[0], "row", newPane());
    const removed = removePane(root, paneIds(root)[1]);
    expect(removed).not.toBeNull();
    sumsToOne(removed as Node);
  });

  it("returns null when the last pane goes, rather than an empty split", () => {
    const root = newPane();
    expect(removePane(root, root.id)).toBeNull();
  });
});

describe("resizeSplit", () => {
  it("moves the boundary and disturbs nothing else", () => {
    let root: Node = newPane();
    root = splitPane(root, root.id, "row", newPane());
    root = splitPane(root, paneIds(root)[2] ?? paneIds(root)[1], "row", newPane());
    const split = root as SplitNode;
    const before = [...split.sizes];

    const resized = resizeSplit(root, split.id, 0, 0.1) as SplitNode;
    expect(resized.sizes[0]).toBeCloseTo(before[0] + 0.1, 10);
    expect(resized.sizes[1]).toBeCloseTo(before[1] - 0.1, 10);
    expect(resized.sizes[2]).toBeCloseTo(before[2], 10);
    sumsToOne(resized);
  });

  it("clamps at the minimum rather than letting a pane vanish", () => {
    const leaf = newPane();
    const root = splitPane(leaf, leaf.id, "row", newPane()) as SplitNode;
    const resized = resizeSplit(root, root.id, 0, 5) as SplitNode;
    expect(resized.sizes[1]).toBeCloseTo(MIN_PANE_FRACTION, 10);
    sumsToOne(resized);
  });
});

describe("equalize and swap", () => {
  it("resets every level to even shares", () => {
    let root: Node = newPane();
    root = splitPane(root, root.id, "row", newPane());
    root = splitPane(root, paneIds(root)[0], "col", newPane());
    const evened = equalize(resizeSplit(root, (root as SplitNode).id, 0, 0.2));
    sumsToOne(evened);
    expect((evened as SplitNode).sizes).toEqual(evenSizes(2));
  });

  it("swaps two panes in place", () => {
    const leaf = newPane();
    let root: Node = splitPane(leaf, leaf.id, "row", newPane());
    const [a, b] = paneIds(root);
    root = setWidget(root, a, chat("/a"));
    root = swapPanes(root, a, b);
    expect(paneIds(root)).toEqual([b, a]);
    expect(findPane(root, a)?.widget).toEqual(chat("/a"));
  });
});

describe("directional navigation", () => {
  /** A row of two, whose right child is a column of two. */
  function layout() {
    const leaf = newPane();
    let root: Node = splitPane(leaf, leaf.id, "row", newPane());
    const right = paneIds(root)[1];
    root = splitPane(root, right, "col", newPane());
    const [left, topRight, bottomRight] = paneIds(root);
    return { root, left, topRight, bottomRight };
  }

  it("crosses a split to the neighbour on that side", () => {
    const { root, left, topRight } = layout();
    expect(neighbour(root, left, "right")).toBe(topRight);
    expect(neighbour(root, topRight, "left")).toBe(left);
  });

  it("moves within a column", () => {
    const { root, topRight, bottomRight } = layout();
    expect(neighbour(root, topRight, "down")).toBe(bottomRight);
    expect(neighbour(root, bottomRight, "up")).toBe(topRight);
  });

  it("returns null at the edge of the window", () => {
    const { root, left } = layout();
    expect(neighbour(root, left, "left")).toBeNull();
    expect(neighbour(root, left, "up")).toBeNull();
  });

  it("enters a sibling from the side it was crossed", () => {
    // Moving left out of the right-hand column lands on the left pane, not
    // some arbitrary leaf.
    const { root, bottomRight, left } = layout();
    expect(neighbour(root, bottomRight, "left")).toBe(left);
  });

  it("cycles in reading order and wraps", () => {
    const { root, left, topRight, bottomRight } = layout();
    expect(cyclePane(root, left, 1)).toBe(topRight);
    expect(cyclePane(root, bottomRight, 1)).toBe(left);
    expect(cyclePane(root, left, -1)).toBe(bottomRight);
  });

  it("addresses panes by 1-based ordinal, matching the number overlay", () => {
    const { root, left, bottomRight } = layout();
    expect(paneByOrdinal(root, 1)).toBe(left);
    expect(paneByOrdinal(root, 3)).toBe(bottomRight);
    expect(paneByOrdinal(root, 4)).toBeNull();
    expect(ordinalOfPane(root, bottomRight)).toBe(3);
  });
});

describe("panes", () => {
  it("lists in depth-first reading order", () => {
    const leaf = newPane();
    let root: Node = splitPane(leaf, leaf.id, "row", newPane());
    root = splitPane(root, paneIds(root)[0], "col", newPane());
    expect(panes(root)).toHaveLength(3);
    expect(paneIds(root)).toEqual(panes(root).map((p) => p.id));
  });
});

describe("swapWidgets", () => {
  it("exchanges what two panes show and leaves the pane ids where they are", () => {
    const a = newPane(chat("/a"));
    const b = newPane(chat("/b"));
    const root: Node = { type: "split", id: "s" as never, dir: "row", children: [a, b], sizes: [0.5, 0.5] };

    const next = swapWidgets(root, a.id, b.id);
    const [first, second] = panes(next);

    expect(first.id).toBe(a.id);
    expect(second.id).toBe(b.id);
    expect(first.widget).toEqual(chat("/b"));
    expect(second.widget).toEqual(chat("/a"));
    sumsToOne(next);
  });

  it("moves a widget into an empty pane", () => {
    const a = newPane(chat("/a"));
    const b = newPane(null);
    const root: Node = { type: "split", id: "s" as never, dir: "row", children: [a, b], sizes: [0.5, 0.5] };

    const [first, second] = panes(swapWidgets(root, a.id, b.id));
    expect(first.widget).toBeNull();
    expect(second.widget).toEqual(chat("/a"));
  });

  it("is one traversal — swapping never leaves two panes showing the same widget", () => {
    // A two-pass implementation writes A's widget into B and then reads it back
    // out again, so both panes end up on A.
    const a = newPane(chat("/a"));
    const b = newPane(chat("/b"));
    const root: Node = { type: "split", id: "s" as never, dir: "row", children: [a, b], sizes: [0.5, 0.5] };
    const [first, second] = panes(swapWidgets(root, a.id, b.id));
    expect(first.widget).not.toEqual(second.widget);
  });

  it("is a no-op for the same pane, an unknown pane, or two empty panes", () => {
    const a = newPane(chat("/a"));
    const b = newPane(null);
    const c = newPane(null);
    const root: Node = { type: "split", id: "s" as never, dir: "row", children: [a, b, c], sizes: [0.4, 0.3, 0.3] };

    expect(swapWidgets(root, a.id, a.id)).toBe(root);
    expect(swapWidgets(root, a.id, "nope" as PaneId)).toBe(root);
    expect(swapWidgets(root, b.id, c.id)).toBe(root);
  });
});
