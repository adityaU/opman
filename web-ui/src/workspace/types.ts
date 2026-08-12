/**
 * The desktop workspace: windows, panes and widgets.
 *
 * A window is a tree of panes; a pane holds at most one widget. The tree is
 * n-ary rather than binary — three panes in a row is one split with three
 * children, not two nested splits — because repeated same-direction splitting
 * is the common case and a binary tree turns it into unbounded nesting that
 * every traversal then has to pay for.
 *
 * The shapes here are written so the illegal states cannot be spelled: a
 * widget's payload lives in its own arm of the union, so a terminal can never
 * carry a session id, and the two id kinds are branded so a `WindowId` cannot
 * be passed where a `PaneId` belongs.
 */

import type { FileOpenRequest } from "../code-editor/types";

export type { FileOpenRequest };

// ── Ids ─────────────────────────────────────────────────

declare const paneBrand: unique symbol;
declare const windowBrand: unique symbol;
declare const splitBrand: unique symbol;

export type PaneId = string & { readonly [paneBrand]: true };
export type WindowId = string & { readonly [windowBrand]: true };
export type SplitId = string & { readonly [splitBrand]: true };

export const asPaneId = (raw: string): PaneId => raw as PaneId;
export const asWindowId = (raw: string): WindowId => raw as WindowId;
export const asSplitId = (raw: string): SplitId => raw as SplitId;

// ── Widgets ─────────────────────────────────────────────

export type WidgetKind = "chat" | "files" | "terminal" | "git";

export const WIDGET_KINDS: readonly WidgetKind[] = ["chat", "files", "terminal", "git"];

/**
 * The engine a chat pane sends on: which runner, and how that runner is set up.
 *
 * A pane owns this rather than the app because two panes side by side are
 * routinely two different jobs — a Codex refactor next to a Claude review — and
 * a single global choice makes the second one silently run on the first one's
 * engine. `null` on a pane means "follow the shell", which is what a pane the
 * user has never configured should do.
 *
 * Model, effort and permission all mean something only inside one runner, so
 * they travel together with it and are never merged across two.
 */
export interface PaneEngine {
  readonly runner: string;
  readonly model: { readonly providerID: string; readonly modelID: string } | null;
  readonly agent: string;
  readonly effort: string | null;
  readonly permission: string;
}

/**
 * What a pane is showing. Tagged by `kind`, with each arm carrying only the
 * state that kind actually has.
 *
 * `projectPath` is on every arm rather than on the pane: a pane's project is a
 * property of what it is showing, and moving a widget between panes must carry
 * the project with it.
 */
export type WidgetState =
  | {
      readonly kind: "chat";
      readonly projectPath: string;
      readonly sessionId: string | null;
      /** Null until the pane's engine is changed away from the shell's. */
      readonly engine: PaneEngine | null;
    }
  /**
   * `open` is the last file the pane was asked to reveal, carried on the widget
   * so a reload comes back to it. Browsing inside the panel does not write here
   * — this is the request, not the panel's cursor.
   */
  | {
      readonly kind: "files";
      readonly projectPath: string;
      /** The pane-local Neovim pool key used when this surface enters Vim mode. */
      readonly sessionId: string;
      readonly open: FileOpenRequest | null;
    }
  /**
   * `ptyIds` are the pane's terminal tabs. PTYs live in the server process and
   * outlive a browser refresh, so remembering their ids is the whole of what it
   * takes for a terminal to survive one — the pane re-attaches to the running
   * shell instead of spawning a fresh one and losing the scrollback.
   */
  | { readonly kind: "terminal"; readonly projectPath: string; readonly ptyIds: readonly string[] }
  | { readonly kind: "git"; readonly projectPath: string }

/** Build a widget once its destination pane — and therefore its identity — is known. */
export type WidgetForPane = (pane: PaneId) => WidgetState;

// ── Tree ────────────────────────────────────────────────

export type SplitDir = "row" | "col";

export interface PaneNode {
  readonly type: "leaf";
  readonly id: PaneId;
  /** An empty pane shows the inline widget opener rather than nothing. */
  readonly widget: WidgetState | null;
}

/**
 * Invariants, enforced by `tree.ts` and asserted in its tests:
 * `children.length === sizes.length`, `children.length >= 2`, and `sizes` sums
 * to 1. A split that would fall to one child collapses into that child.
 */
export interface SplitNode {
  readonly type: "split";
  readonly id: SplitId;
  readonly dir: SplitDir;
  readonly children: readonly Node[];
  readonly sizes: readonly number[];
}

export type Node = PaneNode | SplitNode;

// ── Windows and the workspace ───────────────────────────

export interface WorkspaceWindow {
  readonly id: WindowId;
  readonly name: string;
  readonly root: Node;
  readonly focusedPaneId: PaneId;
  /** Zoomed pane, rendered alone at full size. tmux's `prefix z`. */
  readonly zoomedPaneId: PaneId | null;
}

/**
 * The hideable chrome the workspace owns.
 *
 * The sidebar is deliberately absent: `usePanelState` already owns it and
 * `mod+b` already toggles it, and a second copy here would be a second source
 * of truth for one boolean — the kind that drifts and then disagrees.
 */
export interface ChromeState {
  readonly rail: boolean;
  readonly paneHeaders: boolean;
  /**
   * One pane, the whole shell, nothing else. Distinct from `rail` and
   * `paneHeaders`, which are durable preferences — Zen is a mode you are
   * currently in, and it ends when the zoom it rides on does.
   */
  readonly zen: boolean;
}

export interface Workspace {
  readonly windows: readonly WorkspaceWindow[];
  readonly activeWindowId: WindowId;
  readonly chrome: ChromeState;
}

export const DEFAULT_CHROME: ChromeState = {
  rail: true,
  paneHeaders: true,
  zen: false,
};

// ── Geometry ────────────────────────────────────────────

/** Below this fraction a pane is unusable, so drags and splits clamp to it. */
export const MIN_PANE_FRACTION = 0.08;

export type Direction = "left" | "right" | "up" | "down";

export const DIRECTION_AXIS: Readonly<Record<Direction, SplitDir>> = {
  left: "row",
  right: "row",
  up: "col",
  down: "col",
};

/** Whether a direction moves toward lower indices within its split. */
export const DIRECTION_IS_BACKWARD: Readonly<Record<Direction, boolean>> = {
  left: true,
  up: true,
  right: false,
  down: false,
};
