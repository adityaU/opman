/**
 * A portal-anchored popover.
 *
 * The git panel sits inside scrolling, `overflow`-clipping ancestors, so an
 * absolutely positioned dropdown gets cut off at the panel edge. Rendering to
 * `document.body` and positioning against the trigger's bounding rect is the
 * only placement that survives that, and putting it here means every popover
 * in the panel closes on outside-mousedown and Escape identically.
 */

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { ReactNode, RefObject } from "react";

export interface PopoverProps {
  /** The control the panel is anchored beneath. */
  anchor: RefObject<HTMLElement | null>;
  onClose: () => void;
  label: string;
  children: ReactNode;
}

interface Placement {
  top: number;
  left: number;
  width: number;
}

const MARGIN = 8;
const MIN_WIDTH = 240;
const MAX_WIDTH = 380;

export function Popover({ anchor, onClose, label, children }: PopoverProps) {
  const surface = useRef<HTMLDivElement | null>(null);
  const [place, setPlace] = useState<Placement | null>(null);

  const measure = useCallback(() => {
    const trigger = anchor.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const width = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, rect.width));
    // Flip toward whichever side has room rather than letting the surface run
    // off the viewport, which on a narrow panel is the common case.
    const left = Math.max(MARGIN, Math.min(rect.left, window.innerWidth - width - MARGIN));
    const below = rect.bottom + 6;
    const height = surface.current?.offsetHeight ?? 0;
    const flip = height > 0 && below + height > window.innerHeight - MARGIN && rect.top > height;
    setPlace({ top: flip ? rect.top - height - 6 : below, left, width });
  }, [anchor]);

  useLayoutEffect(() => {
    measure();
  }, [measure]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    const onDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (surface.current?.contains(target)) return;
      if (anchor.current?.contains(target)) return;
      onClose();
    };
    const onReflow = () => measure();

    document.addEventListener("keydown", onKey, true);
    document.addEventListener("mousedown", onDown, true);
    window.addEventListener("resize", onReflow);
    window.addEventListener("scroll", onReflow, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("resize", onReflow);
      window.removeEventListener("scroll", onReflow, true);
    };
  }, [anchor, measure, onClose]);

  return createPortal(
    <div
      ref={surface}
      className="gitp-popover"
      role="dialog"
      aria-label={label}
      style={
        place
          ? { top: `${place.top}px`, left: `${place.left}px`, width: `${place.width}px` }
          : { top: "-9999px", left: "-9999px" }
      }
    >
      {children}
    </div>,
    document.body,
  );
}
