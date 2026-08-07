import React, { useLayoutEffect, useRef, useState } from "react";
import { Gutter } from "./Gutter";
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

export const PaneTree: React.FC<PaneTreeProps> = function PaneTree(props) {
  const { node, zoomedPaneId } = props;

  if (zoomedPaneId) {
    const zoomed = findLeaf(node, zoomedPaneId);
    if (zoomed) return <div className="wsp-tree wsp-tree-zoomed">{props.renderPane(zoomed)}</div>;
  }
  return <Branch {...props} node={node} depth={0} />;
};

interface BranchProps extends PaneTreeProps {
  readonly depth: number;
}

const Branch: React.FC<BranchProps> = function Branch({ node, depth, ...rest }) {
  if (node.type === "leaf") return <>{rest.renderPane(node)}</>;

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

    const measure = () => {
      const box = element.getBoundingClientRect();
      setExtent(dir === "row" ? box.width : box.height);
    };
    measure();

    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref, dir]);

  return extent;
}
