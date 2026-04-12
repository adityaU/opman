import { useCallback, useRef, useState, useEffect, useMemo } from "react";

interface UseResizableOptions {
  /** Initial size in pixels */
  initialSize: number;
  /** Minimum size in pixels */
  minSize?: number;
  /** Maximum size in pixels */
  maxSize?: number;
  /** Direction: "horizontal" for left-right drag, "vertical" for top-bottom */
  direction?: "horizontal" | "vertical";
  /** Whether dragging from the "end" side (right/bottom) instead of start */
  reverse?: boolean;
}

interface UseResizableResult {
  /** Current size in pixels */
  size: number;
  /** Set size programmatically */
  setSize: (s: number) => void;
  /** Props to spread on the drag handle element */
  handleProps: {
    onMouseDown: (e: React.MouseEvent) => void;
    onTouchStart: (e: React.TouchEvent) => void;
    className: string;
    style: React.CSSProperties;
  };
  /** Whether currently dragging */
  isDragging: boolean;
}

/** Extract position from either a MouseEvent or TouchEvent */
function getPointerPos(
  e: MouseEvent | TouchEvent,
  direction: "horizontal" | "vertical"
): number {
  if ("touches" in e) {
    const touch = e.touches[0] ?? (e as TouchEvent).changedTouches[0];
    return direction === "horizontal" ? touch.clientX : touch.clientY;
  }
  return direction === "horizontal"
    ? (e as MouseEvent).clientX
    : (e as MouseEvent).clientY;
}

/**
 * Hook for making a panel resizable via a drag handle.
 * Attach `handleProps` to a div that acts as the drag separator.
 * Supports both mouse and touch (iPad/tablet) interactions.
 */
export function useResizable({
  initialSize,
  minSize = 150,
  maxSize = 800,
  direction = "horizontal",
  reverse = false,
}: UseResizableOptions): UseResizableResult {
  const [size, setSize] = useState(initialSize);
  const [isDragging, setIsDragging] = useState(false);
  const startPosRef = useRef(0);
  const startSizeRef = useRef(0);
  /** Track whether the current drag started from touch */
  const isTouchRef = useRef(false);

  const startDrag = useCallback(
    (clientPos: number, isTouch: boolean) => {
      setIsDragging(true);
      isTouchRef.current = isTouch;
      startPosRef.current = clientPos;
      startSizeRef.current = size;
    },
    [size]
  );

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const pos = direction === "horizontal" ? e.clientX : e.clientY;
      startDrag(pos, false);
    },
    [direction, startDrag]
  );

  const onTouchStart = useCallback(
    (e: React.TouchEvent) => {
      // Don't prevent default here — let the browser know we'll handle it
      // via touch-action CSS instead to avoid passive-listener warnings.
      const touch = e.touches[0];
      const pos = direction === "horizontal" ? touch.clientX : touch.clientY;
      startDrag(pos, true);
    },
    [direction, startDrag]
  );

  useEffect(() => {
    if (!isDragging) return;

    const isTouch = isTouchRef.current;

    const onMove = (e: MouseEvent | TouchEvent) => {
      const currentPos = getPointerPos(e, direction);
      const delta = currentPos - startPosRef.current;
      const newSize = reverse
        ? startSizeRef.current - delta
        : startSizeRef.current + delta;
      setSize(Math.max(minSize, Math.min(maxSize, newSize)));
    };

    const onEnd = () => {
      setIsDragging(false);
    };

    if (isTouch) {
      document.addEventListener("touchmove", onMove, { passive: true });
      document.addEventListener("touchend", onEnd);
      document.addEventListener("touchcancel", onEnd);
    } else {
      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onEnd);
    }

    // Prevent text selection during drag
    document.body.style.userSelect = "none";
    if (!isTouch) {
      document.body.style.cursor =
        direction === "horizontal" ? "col-resize" : "row-resize";
    }

    return () => {
      if (isTouch) {
        document.removeEventListener("touchmove", onMove);
        document.removeEventListener("touchend", onEnd);
        document.removeEventListener("touchcancel", onEnd);
      } else {
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onEnd);
      }
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
    };
  }, [isDragging, direction, minSize, maxSize, reverse]);

  const handleProps = useMemo(() => ({
    onMouseDown,
    onTouchStart,
    className: `resize-handle resize-handle-${direction}${isDragging ? " dragging" : ""}`,
    style: {
      cursor: direction === "horizontal" ? "col-resize" : "row-resize",
      touchAction: "none" as const,
    } as React.CSSProperties,
  }), [onMouseDown, onTouchStart, direction, isDragging]);

  return { size, setSize, handleProps, isDragging };
}
