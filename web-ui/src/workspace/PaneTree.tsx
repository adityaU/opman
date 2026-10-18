import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Gutter } from "./Gutter";
import { panes as panesOf } from "./tree";
import type { Node, PaneId, PaneNode, SplitId } from "./types";

/**
 * Renders a window's tree.
 *
 * Sizes are flex-basis percentages rather than a CSS grid: a grid would need
 * its template rewritten on every drag frame, while `flex-basis` on the
 * children leaves the gutters at their natural width and lets the browser do
 * the arithmetic. `min-width/height: 0` on every child is what stops a wide
 * terminal or a long file name from refusing to shrink.
 */

interface PaneTreeProps {
  readonly node: Node;
  readonly renderPane: (pane: PaneNode) => React.ReactNode;
  readonly onResize: (split: SplitId, index: number, delta: number) => void;
  readonly onEqualize: () => void;
  /** Rendered alone at full size when set — tmux's zoom. */
  readonly zoomedPaneId: PaneId | null;
}

/**
 * A pane's content lives in a host element of its own, created once per pane and rendered
 * into through a portal. The tree only decides where the host sits (a [`Slot`]).
 *
 * That is what keeps a pane alive when the layout changes around it. React remounts
 * anything whose parent chain changes, and splitting a pane wraps it in a new split: its
 * chat, terminal or editor would be thrown away and rebuilt. With the content in a stable
 * portal the wrapper can change freely; only the host element is moved in the DOM.
 */
export const PaneTree: React.FC<PaneTreeProps> = function PaneTree(props) {
  const { node, zoomedPaneId, renderPane } = props;
  const hosts = useRef(new Map<PaneId, HTMLDivElement>());
  const hostOf = (id: PaneId): HTMLDivElement => {
    const known = hosts.current.get(id);
    if (known) return known;
    const created = document.createElement("div");
    created.style.display = "contents";
    hosts.current.set(id, created);
    return created;
  };

  const zoomed = zoomedPaneId ? findLeaf(node, zoomedPaneId) : null;
  const shown = zoomed ? [zoomed] : panesOf(node);

  // Forget the hosts of panes that no longer exist.
  useEffect(() => {
    const alive = new Set(panesOf(node).map((pane) => pane.id));
    for (const id of hosts.current.keys()) {
      if (!alive.has(id)) hosts.current.delete(id);
    }
  }, [node]);

  return (
    <>
      {zoomed ? (
        <div className="wsp-tree wsp-tree-zoomed">
          <Slot host={hostOf(zoomed.id)} />
        </div>
      ) : (
        <Branch {...props} node={node} depth={0} hostOf={hostOf} />
      )}
      {shown.map((pane) => createPortal(renderPane(pane), hostOf(pane.id), pane.id))}
    </>
  );
};

/** Where a pane's host sits in the layout. Moves the host in; never owns its content. */
const Slot: React.FC<{ readonly host: HTMLElement }> = function Slot({ host }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const slot = ref.current;
    if (slot && host.parentNode !== slot) slot.appendChild(host);
  }, [host]);
  return <div ref={ref} className="wsp-slot" style={{ display: "contents" }} />;
};

interface BranchProps extends PaneTreeProps {
  readonly depth: number;
  readonly hostOf: (id: PaneId) => HTMLElement;
}

const Branch: React.FC<BranchProps> = function Branch({ node, depth, ...rest }) {
  if (node.type === "leaf") return <Slot host={rest.hostOf(node.id)} />;

  return <Split node={node} depth={depth} {...rest} />;
};

const Split: React.FC<BranchProps & { node: Extract<Node, { type: "split" }> }> = function Split({
  node,
  depth,
  ...rest
}) {
  const ref = useRef<HTMLDivElement>(null);
  const extent = useExtent(ref, node.dir);

  return (
    <div ref={ref} className={`wsp-split wsp-split-${node.dir}`} data-depth={depth}>
      {node.children.map((child, index) => (
        <React.Fragment key={child.id}>
          {index > 0 && (
            <Gutter
              split={node.id}
              index={index - 1}
              dir={node.dir}
              extent={extent}
              label={`Resize pane ${index} and ${index + 1}`}
              onResize={rest.onResize}
              onEqualize={rest.onEqualize}
            />
          )}
          <div className="wsp-split-child" style={{ flexBasis: `${node.sizes[index] * 100}%` }}>
            <Branch node={child} depth={depth + 1} {...rest} />
          </div>
        </React.Fragment>
      ))}
    </div>
  );
};

function findLeaf(node: Node, id: PaneId): PaneNode | null {
  if (node.type === "leaf") return node.id === id ? node : null;
  for (const child of node.children) {
    const found = findLeaf(child, id);
    if (found) return found;
  }
  return null;
}

/**
 * The split's own width or height in px, which the gutter needs to turn a
 * pointer delta into a fraction.
 *
 * Measured with a ResizeObserver rather than read on each pointer move: a
 * layout read inside a move handler forces a synchronous reflow on every
 * frame of the drag, which is exactly when it is least affordable.
 */
function useExtent(ref: React.RefObject<HTMLDivElement>, dir: "row" | "col"): number {
  const [extent, setExtent] = useState(0);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;

    // A zero extent is never a real layout — it is a subtree that is not being
    // rendered right now (a background window skips its contents entirely).
    // Keeping the last real number is what lets the tree come back with its
    // proportions intact instead of every gutter snapping to nothing.
    const measure = () => {
      const box = element.getBoundingClientRect();
      const next = dir === "row" ? box.width : box.height;
      if (next > 0) setExtent(next);
    };
    measure();

    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref, dir]);

  return extent;
}
