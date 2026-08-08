import React, { useEffect, useRef } from "react";
import type { PaneId } from "./types";

/**
 * One window's pane tree, kept alive whether or not it is the visible one.
 *
 * Switching windows used to unmount the whole tree and mount the next, which
 * threw away every piece of state the widgets held that was not in the
 * workspace reducer: half-typed composer drafts first of all, but also
 * transcript scroll position, editor cursor and selection, terminal viewport,
 * and every open SSE stream. Coming back to a window gave you a fresh one
 * wearing its name.
 *
 * So inactive windows stay mounted and are hidden instead. `visibility` rather
 * than `display: none` on purpose: a display-none subtree has no box, and both
 * xterm and the editor measure themselves from their container, so they would
 * come back sized to nothing and need a refit on every switch. Hidden-but-laid-
 * out costs a little layout and no paint, and needs no refit at all.
 *
 * This layer is also the only thing in the workspace that knows which window is
 * on screen. Nothing below it does — `WindowView` is memoised on its own window
 * and would re-render every mounted tree if visibility were a prop of the tree
 * rather than of the layer around it. The cost of that lands on the switch,
 * which is the one moment it is most visible.
 *
 * The hidden copies need nothing else to keep them honest. `visibility: hidden`
 * already takes their contents out of the tab order and out of the
 * accessibility tree, so a screen reader hears one window, not three. This used
 * to also toggle `inert`, which says the same thing a second time and is not
 * free: changing inertness invalidates style for the whole subtree, and on a
 * workspace holding three chat transcripts that was ~60ms of recalc and repaint
 * per switch — the single largest cost of changing windows.
 */

interface WindowLayerProps {
  readonly active: boolean;
  /** The pane DOM focus moves to when this window becomes the visible one. */
  readonly focusedPaneId: PaneId;
  readonly children: React.ReactNode;
}

export const WindowLayer: React.FC<WindowLayerProps> = function WindowLayer({
  active,
  focusedPaneId,
  children,
}) {
  const ref = useRef<HTMLDivElement>(null);

  // Read through a ref: focus is adopted when the window is *activated*, and
  // moving the focus within an already-visible window is the pane's own job.
  const focusedRef = useRef(focusedPaneId);
  focusedRef.current = focusedPaneId;

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    if (!active) return;

    // The pane cannot do this itself. Its `focused` prop does not change when
    // the window is switched to — it was already its window's focused pane —
    // so nothing over there re-runs, which is exactly what keeps the switch
    // cheap. Adopting focus here is the one thing that has to happen anyway.
    if (element.contains(document.activeElement)) return;
    const pane = element.querySelector<HTMLElement>(
      `[data-pane-id="${CSS.escape(focusedRef.current)}"]`,
    );
    pane?.focus({ preventScroll: true });
  }, [active]);

  return (
    <div ref={ref} className={`wsp-window${active ? " is-active" : ""}`} aria-hidden={!active}>
      {children}
    </div>
  );
};
