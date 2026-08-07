/**
 * Structural operations on a window's pane tree.
 *
 * Every function is pure and returns a new tree, sharing the untouched
 * subtrees. The invariants from `types.ts` hold on entry and on exit: a split
 * has at least two children, `children` and `sizes` are the same length, and
 * `sizes` sums to 1.
 */

import { uuid } from "../utils/uuid";
import {
  asPaneId,
  asSplitId,
  MIN_PANE_FRACTION,
  type Node,
  type PaneId,
  type PaneNode,
  type SplitDir,
  type SplitId,
  type SplitNode,
  type WidgetState,
} from "./types";

// ── Construction ────────────────────────────────────────

export function newPane(widget: WidgetState | null = null): PaneNode {
  return { type: "leaf", id: asPaneId(uuid()), widget };
}

/** Even sizes for `count` children. The last absorbs the rounding error. */
export function evenSizes(count: number): number[] {
  const share = 1 / count;
  const sizes = Array.from({ length: count }, () => share);
  sizes[count - 1] = 1 - share * (count - 1);
  return sizes;
}

function split(dir: SplitDir, children: readonly Node[], sizes: readonly number[]): SplitNode {
  return { type: "split", id: asSplitId(uuid()), dir, children, sizes };
}

/**
 * Rebuild a split, collapsing it into its only child if it has one. Callers
 * that remove a child go through here so the "at least two children" invariant
 * cannot be broken by a caller forgetting.
 */
function respread(node: SplitNode, children: readonly Node[], sizes: readonly number[]): Node {
  if (children.length === 1) return children[0];
  return { ...node, children, sizes: normalize(sizes) };
}

/** Scale sizes to sum to exactly 1, tolerating drift from repeated edits. */
export function normalize(sizes: readonly number[]): number[] {
  const total = sizes.reduce((sum, size) => sum + size, 0);
  if (total <= 0) return evenSizes(sizes.length);
  return sizes.map((size) => size / total);
}

// ── Queries ─────────────────────────────────────────────

/** Every pane, depth-first — which is also the visual reading order. */
export function panes(node: Node): PaneNode[] {
  if (node.type === "leaf") return [node];
  return node.children.flatMap(panes);
}

export function paneIds(node: Node): PaneId[] {
  return panes(node).map((pane) => pane.id);
}

export function findPane(node: Node, id: PaneId): PaneNode | null {
  return panes(node).find((pane) => pane.id === id) ?? null;
}

export function paneCount(node: Node): number {
  return node.type === "leaf" ? 1 : node.children.reduce((sum, c) => sum + paneCount(c), 0);
}

/** Ancestor splits from the root down to the pane, with the index taken at each. */
export interface PathStep {
  readonly split: SplitNode;
  readonly index: number;
}

export function pathToPane(node: Node, id: PaneId): PathStep[] | null {
  if (node.type === "leaf") return node.id === id ? [] : null;
  for (let index = 0; index < node.children.length; index += 1) {
    const below = pathToPane(node.children[index], id);
    if (below) return [{ split: node, index }, ...below];
  }
  return null;
}

// ── Mutations ───────────────────────────────────────────

/** Replace a pane in place, keeping every other node identical. */
export function mapPane(node: Node, id: PaneId, replace: (pane: PaneNode) => Node): Node {
  if (node.type === "leaf") return node.id === id ? replace(node) : node;
  const children = node.children.map((child) => mapPane(child, id, replace));
  if (children.every((child, i) => child === node.children[i])) return node;
  return { ...node, children };
}

export function setWidget(node: Node, id: PaneId, widget: WidgetState | null): Node {
  return mapPane(node, id, (pane) => (pane.widget === widget ? pane : { ...pane, widget }));
}

/**
 * Split `target`, putting `inserted` after it (or before, for a backward
 * split). When the parent already runs along `dir` the new pane joins it as a
 * sibling instead of nesting a fresh split inside it — that is what keeps
 * "split right" three times from producing three levels of tree.
 */
export function splitPane(
  root: Node,
  target: PaneId,
  dir: SplitDir,
  inserted: PaneNode,
  before = false,
): Node {
  const path = pathToPane(root, target);
  if (!path) return root;

  const parent = path[path.length - 1];
  if (parent && parent.split.dir === dir) {
    return rebuildAlong(root, path, path.length - 1, (node) => {
      const at = parent.index + (before ? 0 : 1);
      const half = node.sizes[parent.index] / 2;
      const sizes = [...node.sizes];
      sizes[parent.index] = half;
      sizes.splice(at, 0, half);
      const children = [...node.children];
      children.splice(at, 0, inserted);
      return { ...node, children, sizes };
    });
  }

  return mapPane(root, target, (pane) =>
    split(dir, before ? [inserted, pane] : [pane, inserted], evenSizes(2)),
  );
}

/**
 * Remove a pane. Returns `null` when it was the window's only one — the caller
 * decides whether that closes the window or reseeds it with an empty pane.
 */
export function removePane(root: Node, target: PaneId): Node | null {
  const path = pathToPane(root, target);
  if (!path) return root;
  if (path.length === 0) return null;

  const parent = path[path.length - 1];
  return rebuildAlong(root, path, path.length - 1, (node) => {
    const children = node.children.filter((_, i) => i !== parent.index);
    const sizes = node.sizes.filter((_, i) => i !== parent.index);
    return respread(node, children, sizes);
  });
}

/** Drop every pane but one, keeping it at full size. tmux's `prefix o`. */
export function onlyPane(root: Node, keep: PaneId): Node {
  return findPane(root, keep) ?? root;
}

export function equalize(node: Node): Node {
  if (node.type === "leaf") return node;
  return {
    ...node,
    children: node.children.map(equalize),
    sizes: evenSizes(node.children.length),
  };
}

/**
 * Move the boundary between children `index` and `index + 1` of a split by
 * `delta` (a fraction of the split's own extent). Only those two children
 * change, so a drag never disturbs the rest of the row.
 */
export function resizeSplit(root: Node, splitId: SplitId, index: number, delta: number): Node {
  return mapSplit(root, splitId, (node) => {
    const before = node.sizes[index];
    const after = node.sizes[index + 1];
    if (before === undefined || after === undefined) return node;
    const room = before + after;
    const next = clamp(before + delta, MIN_PANE_FRACTION, room - MIN_PANE_FRACTION);
    if (next === before) return node;
    const sizes = [...node.sizes];
    sizes[index] = next;
    sizes[index + 1] = room - next;
    return { ...node, sizes };
  });
}

/**
 * Swap two panes in place, keeping both sizes where they are.
 *
 * One traversal, not two `mapPane` calls: after substituting A with B the tree
 * briefly holds two nodes carrying B's id, and a second pass would rewrite both
 * of them.
 */
export function swapPanes(root: Node, a: PaneId, b: PaneId): Node {
  const paneA = findPane(root, a);
  const paneB = findPane(root, b);
  if (!paneA || !paneB || a === b) return root;

  const substitute = (node: Node): Node => {
    if (node.type === "leaf") {
      if (node.id === a) return paneB;
      if (node.id === b) return paneA;
      return node;
    }
    return { ...node, children: node.children.map(substitute) };
  };
  return substitute(root);
}

/**
 * Exchange what two panes are showing, leaving the panes themselves in place.
 *
 * Distinct from `swapPanes`, which moves the nodes. A widget dragged across the
 * screen must not carry its pane's id with it: the id is the focus scope and
 * the key every `data-pane-id` lookup uses, so moving it would make pane 1
 * suddenly be over on the right.
 *
 * One traversal, for the same reason `swapPanes` is one.
 */
export function swapWidgets(root: Node, a: PaneId, b: PaneId): Node {
  if (a === b) return root;
  const paneA = findPane(root, a);
  const paneB = findPane(root, b);
  if (!paneA || !paneB || paneA.widget === paneB.widget) return root;

  const substitute = (node: Node): Node => {
    if (node.type === "leaf") {
      if (node.id === a) return { ...paneA, widget: paneB.widget };
      if (node.id === b) return { ...paneB, widget: paneA.widget };
      return node;
    }
    return { ...node, children: node.children.map(substitute) };
  };
  return substitute(root);
}

// ── Internals ───────────────────────────────────────────

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function mapSplit(node: Node, id: SplitId, replace: (split: SplitNode) => Node): Node {
  if (node.type === "leaf") return node;
  if (node.id === id) return replace(node);
  const children = node.children.map((child) => mapSplit(child, id, replace));
  if (children.every((child, i) => child === node.children[i])) return node;
  return { ...node, children };
}

/**
 * Rewrite the split at `depth` along a known path, rebuilding only the
 * ancestors above it. Cheaper and clearer than a second search from the root.
 */
function rebuildAlong(
  root: Node,
  path: readonly PathStep[],
  depth: number,
  replace: (split: SplitNode) => Node,
): Node {
  const rewritten = replace(path[depth].split);
  let node = rewritten;
  for (let i = depth - 1; i >= 0; i -= 1) {
    const step = path[i];
    const children = [...step.split.children];
    children[step.index] = node;
    node = { ...step.split, children };
  }
  return node;
}
