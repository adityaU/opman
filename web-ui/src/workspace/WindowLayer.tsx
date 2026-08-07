import React, { useEffect, useRef } from "react";

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
 * `inert` is what keeps the hidden copies honest — without it their buttons
 * stay tabbable and their headings stay in the accessibility tree, so a
 * screen reader would hear three windows at once.
 */

interface WindowLayerProps {
  readonly active: boolean;
  readonly children: React.ReactNode;
}

export const WindowLayer: React.FC<WindowLayerProps> = React.memo(function WindowLayer({
  active,
  children,
}) {
  const ref = useRef<HTMLDivElement>(null);

  // Set as a DOM property rather than a JSX prop: React 18 does not know
  // `inert` and would drop it with a warning.
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    if (active) element.removeAttribute("inert");
    else element.setAttribute("inert", "");
  }, [active]);

  return (
    <div ref={ref} className={`wsp-window${active ? " is-active" : ""}`} aria-hidden={!active}>
      {children}
    </div>
  );
});
