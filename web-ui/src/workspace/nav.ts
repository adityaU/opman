/**
 * Directional pane navigation — `Ctrl+K Ctrl+←` in normal mode, `<leader>wh`
 * and `Ctrl-w h` in vim.
 *
 * The rule is the one vim and tmux both use: walk up from the pane until an
 * ancestor splits along the direction's axis *and* has a sibling on that side,
 * then descend into that sibling to the leaf nearest the boundary you crossed.
 * It is purely structural — no geometry, no measuring — which is why it gives
 * the same answer before the first paint as after.
 */

import { panes, pathToPane } from "./tree";
import {
  DIRECTION_AXIS,
  DIRECTION_IS_BACKWARD,
  type Direction,
  type Node,
  type PaneId,
  type SplitDir,
} from "./types";

/**
 * Descend to the leaf adjacent to the boundary just crossed.
 *
 * Along the movement axis, entering from the right means taking the last
 * child; across it, the first child is taken, which reads as "the top-left of
 * what you moved into" and stays predictable without tracking a cursor.
 */
function edgeLeaf(node: Node, axis: SplitDir, fromEnd: boolean): PaneId {
  if (node.type === "leaf") return node.id;
  const index = node.dir === axis && fromEnd ? node.children.length - 1 : 0;
  return edgeLeaf(node.children[index], axis, fromEnd);
}

/** The pane one step in `direction`, or null at the edge of the window. */
export function neighbour(root: Node, from: PaneId, direction: Direction): PaneId | null {
  const path = pathToPane(root, from);
  if (!path) return null;

  const axis = DIRECTION_AXIS[direction];
  const backward = DIRECTION_IS_BACKWARD[direction];

  for (let depth = path.length - 1; depth >= 0; depth -= 1) {
    const { split, index } = path[depth];
    if (split.dir !== axis) continue;
    const sibling = index + (backward ? -1 : 1);
    if (sibling < 0 || sibling >= split.children.length) continue;
    // Moving left means entering the sibling from its right-hand end.
    return edgeLeaf(split.children[sibling], axis, backward);
  }
  return null;
}

/**
 * Cycle through panes in reading order, wrapping. `Ctrl-w w`, and the fallback
 * when a directional move hits the edge of the window.
 */
export function cyclePane(root: Node, from: PaneId, step: 1 | -1): PaneId | null {
  const all = panes(root).map((pane) => pane.id);
  if (all.length === 0) return null;
  const index = all.indexOf(from);
  if (index < 0) return all[0];
  return all[(index + step + all.length) % all.length];
}

/** The nth pane in reading order, 1-based — what `mod+1..9` and the number overlay pick. */
export function paneByOrdinal(root: Node, ordinal: number): PaneId | null {
  const all = panes(root);
  return all[ordinal - 1]?.id ?? null;
}

/** A pane's 1-based position in reading order, for the number overlay's badges. */
export function ordinalOfPane(root: Node, id: PaneId): number | null {
  const index = panes(root).findIndex((pane) => pane.id === id);
  return index < 0 ? null : index + 1;
}

// ── The shell around the tree ───────────────────────────

/**
 * Directional focus does not stop at the pane tree.
 *
 * The sidebar on one side and the window rail (or the legacy right-panel stack)
 * on the other are treated as regions pinned to the tree's horizontal edges:
 * `neighbour` answers first, and only when it reports the edge of the window
 * does focus fall out into the region beyond it. That keeps one rule — "walk up
 * until an ancestor splits along this axis" — as the whole story inside the
 * tree, and makes the shell a strictly outer layer rather than a second,
 * competing notion of adjacency.
 */

export type RegionId = string;

/** Where focus is, across the whole shell rather than just the tree. */
export type ShellFocus =
  | { readonly kind: "pane"; readonly pane: PaneId }
  | { readonly kind: "region"; readonly region: RegionId };

/** The regions flanking the tree, each list in screen order, left to right. */
export interface ShellLayout {
  readonly before: readonly RegionId[];
  readonly after: readonly RegionId[];
}

/**
 * The leaf at one horizontal edge of the tree — where a region hands focus over.
 *
 * `edgeLeaf` along the row axis descends to the last child of every horizontal
 * split when entering from the right and to the first when entering from the
 * left, which is the same "nearest the boundary you crossed" rule an intra-tree
 * move already uses.
 */
export function edgePane(root: Node, side: "left" | "right"): PaneId {
  return edgeLeaf(root, DIRECTION_AXIS.left, side === "right");
}

/**
 * One directional step across the whole shell.
 *
 * `root` may be null when no workspace is mounted — on mobile, or with the
 * board taking over the main area — in which case the regions still navigate
 * between themselves and simply have nothing in the middle.
 */
export function shellNeighbour(
  root: Node | null,
  from: ShellFocus,
  direction: Direction,
  layout: ShellLayout,
): ShellFocus | null {
  if (from.kind === "pane") {
    if (!root) return null;
    const next = neighbour(root, from.pane, direction);
    if (next) return { kind: "pane", pane: next };
    // Vertical moves never leave the tree: nothing is stacked above or below it.
    if (direction === "left") return outermost(layout.before, -1);
    if (direction === "right") return outermost(layout.after, 0);
    return null;
  }

  if (direction === "up" || direction === "down") return null;
  const forward = direction === "right";

  const before = layout.before.indexOf(from.region);
  if (before >= 0) {
    const step = before + (forward ? 1 : -1);
    const sibling = layout.before[step];
    if (sibling) return { kind: "region", region: sibling };
    // Past the last region on the left is the tree itself; past the first is
    // the edge of the window.
    if (!forward || !root) return null;
    return { kind: "pane", pane: edgePane(root, "left") };
  }

  const after = layout.after.indexOf(from.region);
  if (after < 0) return null;
  const step = after + (forward ? 1 : -1);
  const sibling = layout.after[step];
  if (sibling) return { kind: "region", region: sibling };
  if (forward || !root) return null;
  return { kind: "pane", pane: edgePane(root, "right") };
}

function outermost(regions: readonly RegionId[], index: number): ShellFocus | null {
  const region = index < 0 ? regions[regions.length - 1] : regions[index];
  return region ? { kind: "region", region } : null;
}
