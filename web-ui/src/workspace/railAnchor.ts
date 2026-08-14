import { useLayoutEffect, useState } from "react";
import type React from "react";
import type { WindowId } from "./types";

/**
 * Placing a portalled surface beside a window's rail chip.
 *
 * Shared by the rename field and the close confirmation: both are too wide for
 * a 44px rail with `overflow-x: hidden`, so both are portalled to the body and
 * anchored back to the chip they belong to. The rail sits on the right edge of
 * the shell, so opening to the chip's left is the common case rather than the
 * corner one, and both axes clamp to the viewport.
 *
 * Both also have to work when the rail is collapsed to the spine, where there
 * is no chip to point at — hence the fallback origin rather than a null render.
 */

/** Gap between the chip and the surface, on whichever side it opens. */
const GUTTER = 6;
/** Keep-out margin so a clamped surface never touches the viewport edge. */
const EDGE = 8;

export interface Origin {
  readonly top: number;
  readonly left: number;
}

/** Where a surface sits when the rail is collapsed and there is no chip. */
export const FALLBACK_ORIGIN: Origin = { top: 48, left: EDGE };

/**
 * Position beside the window's rail chip, or at the fallback origin.
 *
 * Returns null until the surface has been measured — the caller keeps it
 * hidden for that frame so it is never painted at the pre-clamp position.
 */
export function useRailAnchor(
  windowId: WindowId,
  ref: React.RefObject<HTMLElement | null>,
): Origin | null {
  const [origin, setOrigin] = useState<Origin | null>(null);

  useLayoutEffect(() => {
    const surface = ref.current;
    if (!surface) return;
    const chip = document.querySelector(`[data-window-id="${CSS.escape(windowId)}"]`);
    if (!chip) {
      setOrigin(FALLBACK_ORIGIN);
      return;
    }
    setOrigin(place(chip.getBoundingClientRect(), surface.getBoundingClientRect()));
  }, [ref, windowId]);

  return origin;
}

/**
 * Open to the chip's right when that side fits, otherwise to its left. `top` is
 * the surface's centre: the CSS lifts it by half its own height.
 */
function place(chip: DOMRect, surface: DOMRect): Origin {
  const right = chip.right + GUTTER;
  const fitsRight = right + surface.width <= window.innerWidth - EDGE;
  const left = fitsRight ? right : chip.left - GUTTER - surface.width;
  const half = surface.height / 2;
  return {
    top: clamp(chip.top + chip.height / 2, EDGE + half, window.innerHeight - EDGE - half),
    left: clamp(left, EDGE, Math.max(EDGE, window.innerWidth - EDGE - surface.width)),
  };
}

function clamp(value: number, min: number, max: number): number {
  if (value < min) return min;
  if (value > max) return max;
  return value;
}
