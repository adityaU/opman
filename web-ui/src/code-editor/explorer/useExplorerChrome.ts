/**
 * useExplorerChrome — the explorer's panel behaviour: width dragging, pinning,
 * and the auto-withdraw that an unpinned overlay needs.
 *
 * Unpinned, the explorer is a transient overlay, so it retreats once the
 * pointer leaves it — reading code is never a two-step of dismiss-then-read.
 * Pinned, it is part of the layout and stays exactly where it was put.
 */
import { useCallback, useEffect, useRef, useState } from "react";

const MIN_WIDTH = 160;
const MAX_WIDTH = 480;
/** Long enough to survive the pointer clipping a corner on its way out. */
const HIDE_DELAY_MS = 420;

interface Args {
  /** Anything mid-flight that must keep the panel open regardless of pointer. */
  holdOpen: boolean;
  collapse: () => void;
}

export function useExplorerChrome({ holdOpen, collapse }: Args) {
  const [pinned, setPinned] = useState(false);
  const [width, setWidth] = useState(220);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const cancelHide = useCallback(() => {
    if (!hideTimer.current) return;
    clearTimeout(hideTimer.current);
    hideTimer.current = null;
  }, []);

  const scheduleHide = useCallback(() => {
    if (pinned || holdOpen) return;
    cancelHide();
    hideTimer.current = setTimeout(collapse, HIDE_DELAY_MS);
  }, [pinned, holdOpen, cancelHide, collapse]);

  useEffect(() => cancelHide, [cancelHide]);
  useEffect(() => { if (pinned) cancelHide(); }, [pinned, cancelHide]);

  const onResizeStart = useCallback((event: React.PointerEvent) => {
    event.preventDefault();
    cancelHide();
    const startX = event.clientX;
    const startWidth = width;

    const onMove = (moveEvent: PointerEvent) => {
      const next = startWidth + moveEvent.clientX - startX;
      setWidth(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, next)));
    };
    const onUp = () => {
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    };
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
  }, [width, cancelHide]);

  return {
    pinned,
    togglePinned: useCallback(() => setPinned((value) => !value), []),
    width,
    onResizeStart,
    cancelHide,
    scheduleHide,
  };
}
