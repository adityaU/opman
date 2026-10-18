import React from "react";
import { beforeAll, describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { PaneTree } from "../workspace/PaneTree";
import { asPaneId, asSplitId, type Node, type PaneNode } from "../workspace/types";

const EMPTY_HISTORY = { entries: [], index: -1 } as unknown as PaneNode["history"];
const leaf = (id: string): PaneNode => ({
  type: "leaf",
  id: asPaneId(id),
  widget: null,
  history: EMPTY_HISTORY,
});

/** Counts how many times each pane's content has been created. */
const mounts: Record<string, number> = {};
function Probe({ id }: { id: string }) {
  React.useEffect(() => {
    mounts[id] = (mounts[id] ?? 0) + 1;
  }, [id]);
  return <span data-probe={id} />;
}

const tree = (node: Node) => (
  <PaneTree
    node={node}
    zoomedPaneId={null}
    renderPane={(pane) => <Probe id={pane.id} />}
    onResize={() => undefined}
    onEqualize={() => undefined}
  />
);

describe("PaneTree", () => {
  beforeAll(() => {
    // jsdom has no ResizeObserver; the split measures itself with one.
    globalThis.ResizeObserver ??= class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  });

  it("keeps a pane mounted when a split is wrapped around it", () => {
    for (const key of Object.keys(mounts)) delete mounts[key];
    const a = leaf("a");
    const view = render(tree(a));
    expect(mounts).toEqual({ a: 1 });

    const row: Node = {
      type: "split", id: asSplitId("s1"), dir: "row", children: [a, leaf("b")], sizes: [0.5, 0.5],
    };
    view.rerender(tree(row));
    // The same pane, now one level deeper, then deeper again in the other direction.
    const col: Node = {
      type: "split", id: asSplitId("s2"), dir: "col", children: [row, leaf("c")], sizes: [0.5, 0.5],
    };
    view.rerender(tree(col));

    expect(mounts).toEqual({ a: 1, b: 1, c: 1 });
    expect(view.container.querySelectorAll("[data-probe]")).toHaveLength(3);
  });

  it("puts every pane inside the layout", () => {
    const row: Node = {
      type: "split", id: asSplitId("s"), dir: "row", children: [leaf("a"), leaf("b")], sizes: [0.5, 0.5],
    };
    const view = render(tree(row));
    const children = view.container.querySelectorAll(".wsp-split-child");
    expect(children).toHaveLength(2);
    for (const child of Array.from(children)) {
      expect(child.querySelector("[data-probe]")).not.toBeNull();
    }
  });

  it("drops a pane's content when the pane is removed", () => {
    const row: Node = {
      type: "split", id: asSplitId("s"), dir: "row", children: [leaf("a"), leaf("b")], sizes: [0.5, 0.5],
    };
    const view = render(tree(row));
    view.rerender(tree(leaf("a")));
    expect(view.container.querySelectorAll("[data-probe]")).toHaveLength(1);
  });
});
