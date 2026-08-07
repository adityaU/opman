/**
 * Directional focus across the shell.
 *
 * The interesting cases are all at the seam: the tree's own `neighbour` has to
 * keep answering first, and only its "no" may become a step out into the
 * sidebar or the rail. A region that swallowed a move the tree could have made
 * would make `Ctrl+L` jump to the rail from a left-hand pane.
 */
import { describe, expect, it } from "vitest";
import { edgePane, shellNeighbour, type ShellLayout } from "../workspace/nav";
import { newPane, paneIds, splitPane } from "../workspace/tree";
import type { Node, PaneId } from "../workspace/types";

const SHELL: ShellLayout = { before: ["sidebar"], after: ["rail"] };

/** Three panes side by side, left to right. */
function row(): { root: Node; left: PaneId; middle: PaneId; right: PaneId } {
  let root: Node = newPane();
  root = splitPane(root, paneIds(root)[0], "row", newPane());
  root = splitPane(root, paneIds(root)[1], "row", newPane());
  const [left, middle, right] = paneIds(root);
  return { root, left, middle, right };
}

const pane = (id: PaneId) => ({ kind: "pane", pane: id }) as const;

describe("edgePane", () => {
  it("finds the leaf at each horizontal edge", () => {
    const { root, left, right } = row();
    expect(edgePane(root, "left")).toBe(left);
    expect(edgePane(root, "right")).toBe(right);
  });

  it("descends to the top pane of a stacked edge", () => {
    let root: Node = newPane();
    root = splitPane(root, paneIds(root)[0], "row", newPane());
    const [, rightPane] = paneIds(root);
    root = splitPane(root, rightPane, "col", newPane());
    const ids = paneIds(root);
    // Reading order is left, right-top, right-bottom.
    expect(edgePane(root, "right")).toBe(ids[1]);
  });
});

describe("shellNeighbour inside the tree", () => {
  it("prefers a pane neighbour over stepping out", () => {
    const { root, middle, left, right } = row();
    expect(shellNeighbour(root, pane(middle), "left", SHELL)).toEqual(pane(left));
    expect(shellNeighbour(root, pane(middle), "right", SHELL)).toEqual(pane(right));
  });

  it("falls out to the flanking region only at the edge", () => {
    const { root, left, right } = row();
    expect(shellNeighbour(root, pane(left), "left", SHELL)).toEqual({
      kind: "region",
      region: "sidebar",
    });
    expect(shellNeighbour(root, pane(right), "right", SHELL)).toEqual({
      kind: "region",
      region: "rail",
    });
  });

  it("never leaves the tree vertically", () => {
    const { root, left } = row();
    expect(shellNeighbour(root, pane(left), "up", SHELL)).toBeNull();
    expect(shellNeighbour(root, pane(left), "down", SHELL)).toBeNull();
  });

  it("stays put when the flanking region is not on screen", () => {
    const { root, left } = row();
    const bare: ShellLayout = { before: [], after: [] };
    expect(shellNeighbour(root, pane(left), "left", bare)).toBeNull();
  });
});

describe("shellNeighbour from a region", () => {
  it("enters the tree at the edge it is adjacent to", () => {
    const { root, left, right } = row();
    expect(shellNeighbour(root, { kind: "region", region: "sidebar" }, "right", SHELL)).toEqual(
      pane(left),
    );
    expect(shellNeighbour(root, { kind: "region", region: "rail" }, "left", SHELL)).toEqual(
      pane(right),
    );
  });

  it("stops at the outside edge of the shell", () => {
    const { root } = row();
    expect(shellNeighbour(root, { kind: "region", region: "sidebar" }, "left", SHELL)).toBeNull();
    expect(shellNeighbour(root, { kind: "region", region: "rail" }, "right", SHELL)).toBeNull();
  });

  it("steps between two regions on the same side before reaching the tree", () => {
    const { root, right } = row();
    const stacked: ShellLayout = { before: [], after: ["panels", "rail"] };
    expect(shellNeighbour(root, { kind: "region", region: "rail" }, "left", stacked)).toEqual({
      kind: "region",
      region: "panels",
    });
    expect(shellNeighbour(root, { kind: "region", region: "panels" }, "left", stacked)).toEqual(
      pane(right),
    );
  });

  it("navigates between regions with no tree at all", () => {
    const both: ShellLayout = { before: ["sidebar"], after: ["panels"] };
    expect(shellNeighbour(null, { kind: "region", region: "sidebar" }, "right", both)).toBeNull();
    expect(shellNeighbour(null, { kind: "region", region: "panels" }, "left", both)).toBeNull();
  });

  it("ignores a region it does not know about", () => {
    const { root } = row();
    expect(shellNeighbour(root, { kind: "region", region: "nope" }, "left", SHELL)).toBeNull();
  });
});
