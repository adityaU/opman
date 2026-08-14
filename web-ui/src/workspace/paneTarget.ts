/**
 * Changing what a pane is showing, within a window's tree.
 *
 * Split from `tree.ts`, which is about the tree's *shape* — splitting, removing,
 * resizing, swapping. These four are about a single leaf's contents and its
 * trail, and they are the only operations that have to keep `history.ts`'s
 * invariant: the pane's widget and `entries[index]` are the same object.
 *
 * Pure and total, like everything in `tree.ts`: a pane that is not in the tree,
 * or a step with nowhere to go, returns the tree unchanged.
 */

import {
  amendTarget,
  clearTarget,
  jumpHistory,
  recordTarget,
} from "./history";
import { mapPane } from "./tree";
import type { Node, PaneId, PaneNode, WidgetState } from "./types";

/**
 * Point a pane somewhere, and remember that it went there.
 *
 * The pane's `widget` and its trail's current entry are set to the very same
 * object, which is how the invariant is kept by identity rather than by everyone
 * remembering to keep two copies equal.
 */
export function openWidget(node: Node, id: PaneId, widget: WidgetState | null): Node {
  return mapPane(node, id, (pane) => {
    const history = widget ? recordTarget(pane.history, widget) : clearTarget(pane.history);
    if (pane.widget === widget && history === pane.history) return pane;
    return { ...pane, widget, history };
  });
}

/**
 * Replace what a pane is showing without treating it as a new destination.
 *
 * For a write that adds detail to the place the pane is already on — the session
 * id a chat pane earns on its first send, a per-pane engine change. Recording
 * either would put one conversation in the trail twice.
 */
export function amendWidget(node: Node, id: PaneId, widget: WidgetState | null): Node {
  return mapPane(node, id, (pane) =>
    pane.widget === widget ? pane : { ...pane, widget, history: amendTarget(pane.history, widget) },
  );
}

/**
 * Walk a pane's trail. `step` of -1 is back; `seq` re-arms the entry so a panel
 * that already handled it once acts on it again.
 */
export function stepPaneHistory(node: Node, id: PaneId, step: 1 | -1, seq: number): Node {
  return mapPane(node, id, (pane) => moveTo(pane, pane.history.index + step, seq));
}

export function jumpPaneHistory(node: Node, id: PaneId, index: number, seq: number): Node {
  return mapPane(node, id, (pane) => moveTo(pane, index, seq));
}

function moveTo(pane: PaneNode, index: number, seq: number): PaneNode {
  const moved = jumpHistory(pane.history, index, seq);
  return moved ? { ...pane, widget: moved.widget, history: moved.history } : pane;
}
