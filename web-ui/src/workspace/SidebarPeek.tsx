import React, { useCallback, useEffect, useRef, useState } from "react";

/**
 * The hidden sidebar's hot edge.
 *
 * The sidebar is the single biggest chrome cost (280px), so hiding it is the
 * main lever — but a hidden sidebar you cannot glance at is a sidebar you turn
 * back on and leave on.
 *
 * It peeks as a **floating overlay**, never by reflowing the tree. Widening the
 * layout to slide a panel in would resize every pane, which means refitting
 * every xterm, re-measuring every transcript's scroll anchor and relaying out
 * the editor — on every accidental brush of the left edge. That reflow is what
 * makes most auto-hiding sidebars unpleasant, and avoiding it costs nothing but
 * a `position: fixed`.
 */

/** Long enough to survive crossing the edge on the way somewhere else. */
const OPEN_DELAY_MS = 220;
const CLOSE_DELAY_MS = 260;

interface SidebarPeekProps {
  readonly width: number;
  readonly children: React.ReactNode;
}

export const SidebarPeek: React.FC<SidebarPeekProps> = function SidebarPeek({ width, children }) {
  const [open, setOpen] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  // `inert` is set imperatively: React 18's DOM typings predate it, and the
  // alternative — unmounting the sidebar — would remount live session state and
  // lose its scroll position on every peek.
  useEffect(() => {
    const panel = panelRef.current;
    if (!panel) return;
    if (open) panel.removeAttribute("inert");
    else panel.setAttribute("inert", "");
  }, [open]);

  const schedule = useCallback((next: boolean, delay: number) => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setOpen(next), delay);
  }, []);

  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);

  return (
    <>
      <div
        className="wsp-peek-edge"
        aria-hidden="true"
        onPointerEnter={() => schedule(true, OPEN_DELAY_MS)}
        onPointerLeave={() => schedule(false, CLOSE_DELAY_MS)}
      />
      <div
        ref={panelRef}
        className={`wsp-peek${open ? " is-open" : ""}`}
        style={{ width }}
        onPointerEnter={() => schedule(true, 0)}
        onPointerLeave={() => schedule(false, CLOSE_DELAY_MS)}
      >
        {children}
      </div>
    </>
  );
};
