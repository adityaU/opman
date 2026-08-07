import { useCallback, useEffect } from "react";
import { useCommands } from "./useCommand";
import { SURFACE_ATTRIBUTE } from "./useSurfaceFocus";
import type { CommandId } from "./types";

/**
 * List and tree traversal for a focus surface.
 *
 * One hook rather than a keydown handler per component, for the same reason
 * every other surface registers commands: `j` in the sidebar and `j` in the
 * explorer are two bindings on one behaviour, and the keymap — not the
 * component — is what decides which key reaches it, what `when` scopes it away
 * from a text field, and what the keybindings view is able to show.
 *
 * The rows are found in the DOM by `data-list-item` rather than passed in as a
 * model. A sidebar row, a project header, a task group and a file row are four
 * different components with four different shapes, and the only thing traversal
 * needs from any of them is "you are a stop, here is your depth, here is
 * whether you are open" — which is exactly what ARIA already asks them to say.
 */

/** Which command ids this surface answers to. */
export interface ListNavCommands {
  readonly moveDown: CommandId;
  readonly moveUp: CommandId;
  readonly expand?: CommandId;
  readonly collapse?: CommandId;
  readonly activate?: CommandId;
}

export interface ListNavOptions {
  /** The `data-surface` value naming this list — also how its root is found. */
  readonly surface: string;
  readonly commands: ListNavCommands;
  /** Defaults to clicking the row, which is what the pointer would have done. */
  readonly onActivate?: (item: HTMLElement) => void;
  readonly onExpand?: (item: HTMLElement) => void;
  readonly onCollapse?: (item: HTMLElement) => void;
  readonly enabled?: boolean;
}

const ITEM = "[data-list-item]";

/**
 * Rows a keyboard user can actually reach.
 *
 * `checkVisibility` is the only test that sees a `display: none` *ancestor*
 * without measuring layout — `offsetParent` would do it in a browser but is
 * flatly null under jsdom, which would leave every row in every test invisible.
 * Where it is missing, the markup checks stand alone.
 */
export function isVisible(element: HTMLElement): boolean {
  if (element.closest("[hidden]") || element.closest('[aria-hidden="true"]')) return false;
  const check = (element as HTMLElement & { checkVisibility?: () => boolean }).checkVisibility;
  return typeof check === "function" ? check.call(element) : true;
}

function rootOf(surface: string): HTMLElement | null {
  if (typeof document === "undefined") return null;
  return document.querySelector<HTMLElement>(`[${SURFACE_ATTRIBUTE}="${surface}"]`);
}

function rowsIn(root: HTMLElement | null): HTMLElement[] {
  if (!root) return [];
  return [...root.querySelectorAll<HTMLElement>(ITEM)].filter(isVisible);
}

function depthOf(item: HTMLElement): number {
  return Number(item.dataset.listDepth ?? "0");
}

/** The row focus is on, or the roving stop it was left on. */
function currentRow(root: HTMLElement, rows: readonly HTMLElement[]): HTMLElement | null {
  const active = document.activeElement;
  const focused = active instanceof Element ? active.closest<HTMLElement>(ITEM) : null;
  if (focused && rows.includes(focused)) return focused;
  const stop = root.querySelector<HTMLElement>(`${ITEM}[tabindex="0"]`);
  return stop && rows.includes(stop) ? stop : null;
}

/**
 * Roving tabindex: exactly one row is in the tab order at a time.
 *
 * Without it a long session list or a deep tree costs the reader dozens of Tab
 * presses to step past, and arriving back at the list from elsewhere in the
 * shell lands on its first row rather than the one they left.
 */
function applyRoving(rows: readonly HTMLElement[], chosen: HTMLElement | null): void {
  const stop = chosen && rows.includes(chosen) ? chosen : rows[0];
  for (const row of rows) row.tabIndex = row === stop ? 0 : -1;
}

/** The row that owns `item` — the nearest earlier row one level shallower. */
function parentRow(rows: readonly HTMLElement[], item: HTMLElement): HTMLElement | null {
  const depth = depthOf(item);
  if (depth === 0) return null;
  for (let i = rows.indexOf(item) - 1; i >= 0; i -= 1) {
    if (depthOf(rows[i]) < depth) return rows[i];
  }
  return null;
}

/** jsdom implements neither `scrollIntoView` nor smooth scrolling. */
export function revealElement(element: HTMLElement): void {
  element.scrollIntoView?.({ block: "nearest" });
}

function focusRow(row: HTMLElement | undefined): boolean {
  if (!row) return false;
  row.focus({ preventScroll: true });
  revealElement(row);
  return true;
}

/**
 * Put focus back on a row that a re-render replaced.
 *
 * Opening a folder rebuilds the branch — in the explorer's case the whole
 * subtree, because its row components are declared inside the parent and so are
 * a new component type on every render — and the button that had focus is gone
 * by the time the browser has repainted. Focus lands on `<body>`, and the next
 * `j` starts over from the top of the tree.
 *
 * `data-list-key` is the identity that survives that, and the restore only
 * fires when focus was actually dropped, so a row whose activation legitimately
 * hands focus to an editor keeps it.
 */
function restoreFocus(surface: string, item: HTMLElement): void {
  const key = item.dataset.listKey;
  if (!key || typeof requestAnimationFrame === "undefined") return;

  let tries = 0;
  const tick = () => {
    const active = document.activeElement;
    const lost = !active || active === document.body || active === document.documentElement;
    // Something else took focus deliberately — an editor opening the file that
    // was just activated. Leave it alone.
    if (!lost && active !== item) return;
    if (lost) {
      const row = rootOf(surface)?.querySelector<HTMLElement>(
        `${ITEM}[data-list-key="${CSS.escape(key)}"]`,
      );
      if (row) {
        row.focus({ preventScroll: true });
        return;
      }
    }
    // Still on the doomed node, or the replacement has not landed yet. React
    // commits within a frame or two; a handful of retries covers the gap
    // without leaving a timer running behind a list nobody is using.
    tries += 1;
    if (tries < 8) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

export function useListNav(options: ListNavOptions): void {
  const { surface, commands, onActivate, onExpand, onCollapse } = options;
  const enabled = options.enabled ?? true;

  const step = useCallback(
    (delta: 1 | -1): boolean => {
      const root = rootOf(surface);
      if (!root) return false;
      const rows = rowsIn(root);
      if (rows.length === 0) return false;
      const from = currentRow(root, rows);
      if (!from) return focusRow(delta > 0 ? rows[0] : rows[rows.length - 1]);
      const next = rows[rows.indexOf(from) + delta];
      return focusRow(next);
    },
    [surface],
  );

  /** The row a command acts on: what has focus, else the roving stop. */
  const target = useCallback((): HTMLElement | null => {
    const root = rootOf(surface);
    if (!root) return null;
    const rows = rowsIn(root);
    return currentRow(root, rows) ?? rows[0] ?? null;
  }, [surface]);

  useCommands(
    enabled
      ? {
          [commands.moveDown]: () => step(1),
          [commands.moveUp]: () => step(-1),
          ...(commands.expand
            ? {
                [commands.expand]: () => {
                  const item = target();
                  if (!item) return;
                  // Closed branch opens; open branch descends into its first
                  // child, which is where `l` leaves you in every vim tree.
                  if (item.getAttribute("aria-expanded") === "false") {
                    if (onExpand) onExpand(item);
                    else item.click();
                    restoreFocus(surface, item);
                    return;
                  }
                  if (item.getAttribute("aria-expanded") === "true") step(1);
                },
              }
            : {}),
          ...(commands.collapse
            ? {
                [commands.collapse]: () => {
                  const item = target();
                  if (!item) return;
                  if (item.getAttribute("aria-expanded") === "true") {
                    if (onCollapse) onCollapse(item);
                    else item.click();
                    restoreFocus(surface, item);
                    return;
                  }
                  // A leaf, or an already-closed branch: `h` climbs out.
                  const root = rootOf(surface);
                  if (!root) return;
                  focusRow(parentRow(rowsIn(root), item) ?? undefined);
                },
              }
            : {}),
          ...(commands.activate
            ? {
                [commands.activate]: () => {
                  const item = target();
                  if (!item) return;
                  if (onActivate) onActivate(item);
                  else item.click();
                  restoreFocus(surface, item);
                },
              }
            : {}),
        }
      : {},
  );

  // Rows come and go as branches open, sessions arrive over SSE and React
  // replaces nodes, so the tab order is reapplied from a mutation observer
  // rather than from a render pass this hook does not participate in.
  useEffect(() => {
    if (!enabled || typeof document === "undefined") return undefined;

    let frame = 0;
    const sync = () => {
      frame = 0;
      const root = rootOf(surface);
      if (!root) return;
      const rows = rowsIn(root);
      if (rows.length === 0) return;
      applyRoving(rows, currentRow(root, rows));
    };
    const schedule = () => {
      if (frame) return;
      frame = requestAnimationFrame(sync);
    };

    const observer = new MutationObserver(schedule);
    const attach = () => {
      const root = rootOf(surface);
      if (root) observer.observe(root, { childList: true, subtree: true });
      return root;
    };

    let root = attach();
    if (!root) {
      // The surface may mount after this hook does — the explorer panel is
      // lazy — so retry once on the next frame rather than giving up.
      frame = requestAnimationFrame(() => {
        frame = 0;
        root = attach();
        sync();
      });
    } else {
      sync();
    }

    const onFocusIn = (event: FocusEvent) => {
      const owner = rootOf(surface);
      if (!owner || !(event.target instanceof Element)) return;
      const row = event.target.closest<HTMLElement>(ITEM);
      if (row && owner.contains(row)) applyRoving(rowsIn(owner), row);
    };
    document.addEventListener("focusin", onFocusIn, true);

    return () => {
      observer.disconnect();
      if (frame) cancelAnimationFrame(frame);
      document.removeEventListener("focusin", onFocusIn, true);
    };
  }, [enabled, surface]);
}
