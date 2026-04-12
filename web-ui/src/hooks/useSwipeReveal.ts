/**
 * useSwipeReveal — swipe-left-to-reveal action tray for mobile list rows.
 *
 * Tracks touch gestures on a container element and returns props / style
 * values for the content layer's translateX. Uses refs for internal state
 * (no re-renders during active drag) and useState only for committed state.
 *
 * Usage:
 * ```tsx
 * const swipe = useSwipeReveal({ actionsWidth: 152 });
 * <div className={swipe.containerClass} {...swipe.handlers}>
 *   <div className="swipe-row-actions">{action buttons}</div>
 *   <div className="swipe-row-content" style={swipe.contentStyle}>
 *     {normal row content}
 *   </div>
 * </div>
 * ```
 */

import { useCallback, useRef, useState, useMemo } from "react";
import type { TouchEvent as ReactTouchEvent, CSSProperties } from "react";

export interface SwipeConfig {
  /** Total width (px) of the revealed action tray. */
  actionsWidth: number;
}

export interface SwipeRevealAPI {
  /** Class name(s) for the `.swipe-row` container. */
  containerClass: string;
  /** Inline style for the `.swipe-row-content` layer. */
  contentStyle: CSSProperties;
  /** Touch event handlers to spread on the container element. */
  handlers: {
    onTouchStart: (e: ReactTouchEvent) => void;
    onTouchMove: (e: ReactTouchEvent) => void;
    onTouchEnd: (e: ReactTouchEvent) => void;
    onTouchCancel: (e: ReactTouchEvent) => void;
  };
  /** Whether the action tray is currently open. */
  isOpen: boolean;
  /** Programmatically close the tray. */
  close: () => void;
}

export function useSwipeReveal(config: SwipeConfig): SwipeRevealAPI {
  const maxOffset = -Math.abs(config.actionsWidth);

  // Committed state — only updates on touchEnd / close
  const [offset, setOffset] = useState(0);
  const [swiping, setSwiping] = useState(false);
  const [open, setOpen] = useState(false);

  // Internal refs for real-time tracking (no re-renders during drag)
  const startXRef = useRef(0);
  const startYRef = useRef(0);
  const startOffsetRef = useRef(0);
  const isHorizontalRef = useRef<boolean | null>(null);
  const liveOffsetRef = useRef(0);
  const contentRef = useRef<HTMLElement | null>(null);

  const close = useCallback(() => {
    setOffset(0);
    setOpen(false);
    setSwiping(false);
    liveOffsetRef.current = 0;
  }, []);

  const onTouchStart = useCallback((e: ReactTouchEvent) => {
    const touch = e.touches[0];
    if (!touch) return;
    startXRef.current = touch.clientX;
    startYRef.current = touch.clientY;
    startOffsetRef.current = liveOffsetRef.current;
    isHorizontalRef.current = null;
  }, []);

  const onTouchMove = useCallback((e: ReactTouchEvent) => {
    const touch = e.touches[0];
    if (!touch) return;

    const dx = touch.clientX - startXRef.current;
    const dy = touch.clientY - startYRef.current;

    // Determine direction on first significant move
    if (isHorizontalRef.current === null) {
      const adx = Math.abs(dx);
      const ady = Math.abs(dy);
      if (adx < 5 && ady < 5) return;
      isHorizontalRef.current = adx > ady;
    }

    if (!isHorizontalRef.current) return;

    // Prevent vertical scroll while swiping horizontally
    e.preventDefault();
    setSwiping(true);

    const raw = startOffsetRef.current + dx;
    const clamped = Math.max(maxOffset, Math.min(0, raw));
    liveOffsetRef.current = clamped;

    // Direct DOM update for 60fps (avoid React re-render per frame)
    const el = (e.currentTarget as HTMLElement).querySelector(".swipe-row-content") as HTMLElement | null;
    if (el) {
      el.style.transition = "none";
      el.style.transform = clamped === 0 ? "" : `translateX(${clamped}px)`;
      contentRef.current = el;
    }
  }, [maxOffset]);

  const onTouchEnd = useCallback(() => {
    setSwiping(false);
    const cur = liveOffsetRef.current;
    const threshold = maxOffset * 0.4;

    // Restore CSS transition for snap animation
    if (contentRef.current) {
      contentRef.current.style.transition = "";
    }

    if (cur < threshold) {
      liveOffsetRef.current = maxOffset;
      setOffset(maxOffset);
      setOpen(true);
    } else {
      liveOffsetRef.current = 0;
      setOffset(0);
      setOpen(false);
    }
  }, [maxOffset]);

  const handlers = useMemo(() => ({
    onTouchStart,
    onTouchMove,
    onTouchEnd,
    onTouchCancel: onTouchEnd,
  }), [onTouchStart, onTouchMove, onTouchEnd]);

  const containerClass = `swipe-row${swiping ? " swiping" : ""}${open ? " swipe-open" : ""}`;

  const contentStyle: CSSProperties = offset === 0
    ? {}
    : { transform: `translateX(${offset}px)` };

  return { containerClass, contentStyle, handlers, isOpen: open, close };
}
