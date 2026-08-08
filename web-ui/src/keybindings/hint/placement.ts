/**
 * Where a hover hint sits relative to the control it describes.
 *
 * Split out from the component so the geometry is testable without a DOM: it is
 * the part that goes wrong, and the part a browser makes expensive to check.
 */

export type Placement = "top" | "bottom" | "left" | "right";

export interface Rect {
  readonly top: number;
  readonly left: number;
  readonly width: number;
  readonly height: number;
}

export interface Viewport {
  readonly width: number;
  readonly height: number;
}

/** Distance from the control, and from the edge of the screen. */
const GAP = 8;
const MARGIN = 8;

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(value, max));
}

function centre(anchorStart: number, anchorSize: number, tipSize: number): number {
  return anchorStart + (anchorSize - tipSize) / 2;
}

/**
 * Preferred side first, opposite side if that would run off the screen, then
 * clamped on the other axis.
 *
 * Flipping rather than shrinking is deliberate: a hint that wraps to two lines
 * because it is 4px from an edge is harder to read than the same hint on the
 * other side of the button.
 */
export function place(
  anchor: Rect,
  tip: Rect,
  placement: Placement,
  viewport: Viewport,
): { readonly top: number; readonly left: number } {
  const above = anchor.top - tip.height - GAP;
  const below = anchor.top + anchor.height + GAP;
  const before = anchor.left - tip.width - GAP;
  const after = anchor.left + anchor.width + GAP;

  const fits = {
    top: above >= MARGIN,
    bottom: below + tip.height <= viewport.height - MARGIN,
    left: before >= MARGIN,
    right: after + tip.width <= viewport.width - MARGIN,
  } as const;

  const side: Placement =
    fits[placement] ? placement
    : placement === "top" ? "bottom"
    : placement === "bottom" ? "top"
    : placement === "left" ? "right"
    : "left";

  const top =
    side === "top" ? above
    : side === "bottom" ? below
    : centre(anchor.top, anchor.height, tip.height);

  const left =
    side === "left" ? before
    : side === "right" ? after
    : centre(anchor.left, anchor.width, tip.width);

  return {
    top: clamp(top, MARGIN, Math.max(MARGIN, viewport.height - tip.height - MARGIN)),
    left: clamp(left, MARGIN, Math.max(MARGIN, viewport.width - tip.width - MARGIN)),
  };
}
