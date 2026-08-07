import { useEffect, useRef } from "react";
import { shellNeighbour } from "../workspace/nav";
import { isVisible, revealElement } from "./useListNav";
import type { RegionId, ShellFocus, ShellLayout } from "../workspace/nav";
import type { Direction, Node, PaneId } from "../workspace/types";

/**
 * The shell's directional-navigation registry.
 *
 * `useSurfaceFocus` answers "which surface has focus" from the DOM rather than
 * from React state, and this answers "which region has focus" the same way, for
 * the same reason: focus already lives in the DOM, and a registry keyed off it
 * needs no provider threaded between the sidebar, the workspace and the panels
 * — three subtrees with no common owner below the app root.
 *
 * Regions are declared by selector. The pane tree is the one part that cannot
 * be, because moving inside it is a question about the split tree rather than
 * about the DOM, so the workspace registers a live view of itself instead.
 */

export interface ShellRegion {
  readonly id: RegionId;
  /** Which side of the pane tree the region sits on. */
  readonly side: "before" | "after";
  /** The region's root element. */
  readonly selector: string;
  /**
   * Where focus lands on arrival, tried in order. The first entry is the
   * roving-tabindex stop a list surface leaves behind, so returning to a list
   * returns to the row the user left it on.
   */
  readonly entry: readonly string[];
}

const LIST_STOP = "[data-list-item][tabindex='0']";

/**
 * The desktop shell, outside in. `.right-panel-stack` and `.wsp-rail` never
 * appear together — the first is the pre-workspace layout, the second the
 * workspace one — so their relative order is nominal.
 */
export const SHELL_REGIONS: readonly ShellRegion[] = [
  {
    id: "sidebar",
    side: "before",
    selector: ".chat-sidebar",
    entry: [LIST_STOP, "[data-list-item]", ".sb-session", ".sb-project-header"],
  },
  {
    id: "panels",
    side: "after",
    selector: ".right-panel-stack",
    entry: [LIST_STOP, "[data-list-item]", ".right-panel-tab-trigger", "button"],
  },
  {
    id: "rail",
    side: "after",
    selector: ".wsp-rail",
    entry: [".wsp-rail-chip.is-active", ".wsp-rail-chip", ".wsp-rail-toggle"],
  },
];

/** A live view of the pane tree, published by the workspace. */
export interface NavTree {
  /** Root and focused pane as of right now, or null when nothing is mounted. */
  readonly snapshot: () => { readonly root: Node; readonly focused: PaneId } | null;
  /** Focus a pane, moving real DOM focus into it. */
  readonly focusPane: (pane: PaneId) => void;
}

let tree: NavTree | undefined;

export function registerNavTree(next: NavTree): () => void {
  tree = next;
  return () => {
    if (tree === next) tree = undefined;
  };
}

/**
 * Publish the pane tree for as long as the component is mounted.
 *
 * The spec is read through a ref so a caller may rebuild it every render — the
 * workspace's snapshot closes over state that changes on every focus move.
 */
export function useNavTree(spec: NavTree): void {
  const latest = useRef(spec);
  latest.current = spec;

  useEffect(
    () =>
      registerNavTree({
        snapshot: () => latest.current.snapshot(),
        focusPane: (pane) => latest.current.focusPane(pane),
      }),
    [],
  );
}

function elementOf(region: ShellRegion): HTMLElement | null {
  if (typeof document === "undefined") return null;
  return document.querySelector<HTMLElement>(region.selector);
}

/** A region counts only while it is on screen. */
function onScreen(element: HTMLElement | null): element is HTMLElement {
  return element !== null && isVisible(element);
}

function liveRegions(): ShellRegion[] {
  return SHELL_REGIONS.filter((region) => onScreen(elementOf(region)));
}

function layoutOf(regions: readonly ShellRegion[]): ShellLayout {
  return {
    before: regions.filter((r) => r.side === "before").map((r) => r.id),
    after: regions.filter((r) => r.side === "after").map((r) => r.id),
  };
}

function focusRegion(region: ShellRegion): boolean {
  const root = elementOf(region);
  if (!root) return false;
  for (const selector of region.entry) {
    const target = root.querySelector<HTMLElement>(selector);
    if (!target || !isVisible(target)) continue;
    target.focus({ preventScroll: true });
    revealElement(target);
    return true;
  }
  // Nothing focusable inside is still a move: the region takes focus itself so
  // the next step in the same direction continues from there.
  root.tabIndex = -1;
  root.focus({ preventScroll: true });
  return true;
}

/**
 * Move focus one step in `direction`, across regions and panes alike.
 *
 * Returns false when the move would leave the shell, so the caller can let the
 * key through to whatever has focus rather than swallowing it at the edge.
 */
export function moveFocus(direction: Direction): boolean {
  if (typeof document === "undefined") return false;

  const regions = liveRegions();
  const active = document.activeElement;
  const here = regions.find((region) => elementOf(region)?.contains(active));
  const snapshot = tree?.snapshot() ?? null;

  const from: ShellFocus | null = here
    ? { kind: "region", region: here.id }
    : snapshot
      ? { kind: "pane", pane: snapshot.focused }
      : null;
  if (!from) return false;

  const next = shellNeighbour(snapshot?.root ?? null, from, direction, layoutOf(regions));
  if (!next) return false;

  if (next.kind === "pane") {
    tree?.focusPane(next.pane);
    return true;
  }
  const target = regions.find((region) => region.id === next.region);
  return target ? focusRegion(target) : false;
}
