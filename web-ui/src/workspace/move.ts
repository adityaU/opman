/**
 * Moving a pane to a new place in its own window.
 *
 * A drop is one of two things, and the difference is where in the target pane
 * the pointer let go: on an edge it is a *move* — the pane is lifted out of the
 * tree and re-seated beside the target, and the siblings it left behind spread
 * into its space — while in the middle it is the older *swap*, which leaves
 * both frames where they are and trades what they show.
 *
 * Both are expressed with the operations `tree.ts` already owns, so the tree
 * invariants are enforced in exactly one place: a lift is `removePane`, which
 * collapses a split that falls to one child, and a re-seat is `splitPane`,
 * which joins an existing row rather than nesting a new one inside it.
 */

import { removePane, splitPane, swapWidgets } from "./tree";
import type { Node, PaneId, PaneNode, SplitDir } from "./types";

/**
 * Where in a pane a drop landed. `center` means "trade contents"; the four
 * edges mean "put it on this side of that pane".
 */
export type DropEdge = "left" | "right" | "top" | "bottom" | "center";

export const DROP_EDGES: readonly DropEdge[] = ["left", "right", "top", "bottom", "center"];

const EDGE_AXIS: Readonly<Record<Exclude<DropEdge, "center">, SplitDir>> = {
  left: "row",
  right: "row",
  top: "col",
  bottom: "col",
};

/** Whether the edge seats the pane before the target within its split. */
const EDGE_IS_BEFORE: Readonly<Record<Exclude<DropEdge, "center">, boolean>> = {
  left: true,
  top: true,
  right: false,
  bottom: false,
};

/**
 * Re-seat `moving` against `target`, or trade their widgets for a centre drop.
 *
 * The pane node keeps its id across the move. That id is the focus scope and
 * the key every `data-pane-id` lookup uses — a terminal finds its shell and a
 * files pane its language server through it — so minting a fresh one would turn
 * a move into a close and an open.
 *
 * Returns the same root, cheaply, for every drop that cannot change anything:
 * onto itself, onto a pane that is not there, or out of a window with nothing
 * left to hold the space.
 */
export function movePaneWithin(
  root: Node,
  moving: PaneId,
  target: PaneId,
  edge: DropEdge,
): Node {
  if (moving === target) return root;
  if (edge === "center") return swapWidgets(root, moving, target);

  const lifted = lift(root, moving);
  if (!lifted) return root;

  const seated = splitPane(
    lifted.root,
    target,
    EDGE_AXIS[edge],
    lifted.pane,
    EDGE_IS_BEFORE[edge],
  );
  return seated === lifted.root ? root : seated;
}

/** The pane, and the tree without it. `null` when either half is impossible. */
function lift(root: Node, moving: PaneId): { readonly root: Node; readonly pane: PaneNode } | null {
  const pane = findLeaf(root, moving);
  if (!pane) return null;
  const pruned = removePane(root, moving);
  // `null` is a window down to nothing, and the same node back means the pane
  // was never in this tree.
  if (pruned === null || pruned === root) return null;
  return { root: pruned, pane };
}

function findLeaf(node: Node, id: PaneId): PaneNode | null {
  if (node.type === "leaf") return node.id === id ? node : null;
  for (const child of node.children) {
    const found = findLeaf(child, id);
    if (found) return found;
  }
  return null;
}

/**
 * The edge a pointer is over, from the pane's own box.
 *
 * The bands are a fraction of each side rather than a fixed pixel depth: the
 * gesture has to read the same in a quarter-screen pane and a full-screen one,
 * and a 60px band is most of a small pane and a hairline on a large one. The
 * middle is left generous — swapping two chats is the commonest drop, and it is
 * the one the user aims at least carefully.
 */
const EDGE_BAND = 0.28;

export function edgeFromPointer(box: DOMRectReadOnly, x: number, y: number): DropEdge {
  const left = (x - box.left) / box.width;
  const top = (y - box.top) / box.height;
  // Nearest side wins, so the diagonal corners split evenly instead of the
  // horizontal test quietly claiming both of them.
  const horizontal = Math.min(left, 1 - left);
  const vertical = Math.min(top, 1 - top);
  if (horizontal >= EDGE_BAND && vertical >= EDGE_BAND) return "center";
  if (horizontal <= vertical) return left < 0.5 ? "left" : "right";
  return top < 0.5 ? "top" : "bottom";
}
