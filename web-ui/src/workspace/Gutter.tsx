import React, { useCallback, useEffect, useRef, useState } from "react";
import type { SplitDir, SplitId } from "./types";

/**
 * The boundary between two panes.
 *
 * Unlike `useResizable`, which owns a pixel size for one fixed panel, a gutter
 * owns nothing: it reports a *fraction* of its parent's extent and the tree is
 * the single source of truth. That is what lets a window restore at any
 * viewport size and keep its proportions.
 *
 * It is a real `separator` with `tabIndex`, so the arrow keys resize it and
 * `Home`/`End` snap it — resizing is not a pointer-only capability.
 */

const KEYBOARD_STEP = 0.02;
const KEYBOARD_STEP_FINE = 0.005;

interface GutterProps {
  readonly split: SplitId;
  readonly index: number;
  readonly dir: SplitDir;
  /** Live extent of the parent split, in px, for converting the drag delta. */
  readonly extent: number;
  readonly label: string;
  readonly onResize: (split: SplitId, index: number, delta: number) => void;
  readonly onEqualize: () => void;
}

export const Gutter: React.FC<GutterProps> = React.memo(function Gutter({
  split,
  index,
  dir,
  extent,
  label,
  onResize,
  onEqualize,
}) {
  const [dragging, setDragging] = useState(false);
  const originRef = useRef(0);

  const onPointerDown = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    // Only the primary button drags; right-click belongs to the context menu.
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.focus();
    originRef.current = dir === "row" ? event.clientX : event.clientY;
    setDragging(true);
  }, [dir]);

  useEffect(() => {
    if (!dragging || extent <= 0) return;

    const move = (event: PointerEvent) => {
      const position = dir === "row" ? event.clientX : event.clientY;
      const delta = (position - originRef.current) / extent;
      if (delta === 0) return;
      originRef.current = position;
      onResize(split, index, delta);
    };
    const end = () => setDragging(false);

    // Captured on the document so a fast drag that outruns the 7px hit area
    // keeps tracking, and so releasing outside the window still ends it.
    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", end);
    document.addEventListener("pointercancel", end);
    const previousCursor = document.body.style.cursor;
    document.body.style.cursor = dir === "row" ? "col-resize" : "row-resize";
    document.body.style.userSelect = "none";

    return () => {
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerup", end);
      document.removeEventListener("pointercancel", end);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = "";
    };
  }, [dragging, dir, extent, index, onResize, split]);

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      const step = event.shiftKey ? KEYBOARD_STEP_FINE : KEYBOARD_STEP;
      const back = dir === "row" ? "ArrowLeft" : "ArrowUp";
      const forward = dir === "row" ? "ArrowRight" : "ArrowDown";

      if (event.key === back) onResize(split, index, -step);
      else if (event.key === forward) onResize(split, index, step);
      else if (event.key === "Home" || event.key === "End" || event.key === "Enter") onEqualize();
      else return;

      event.preventDefault();
      event.stopPropagation();
    },
    [dir, index, onEqualize, onResize, split],
  );

  return (
    <div
      role="separator"
      tabIndex={0}
      aria-orientation={dir === "row" ? "vertical" : "horizontal"}
      aria-label={label}
      className={`wsp-gutter wsp-gutter-${dir}${dragging ? " is-dragging" : ""}`}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      onDoubleClick={onEqualize}
    >
      <span className="wsp-gutter-line" aria-hidden="true" />
    </div>
  );
});
