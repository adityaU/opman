import { useCallback, useRef, useState, useEffect } from "react";

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
    className: string;
    style: React.CSSProperties;
  };
  /** Whether currently dragging */
  isDragging: boolean;
}

/**
 * Hook for making a panel resizable via a drag handle.
 * Attach `handleProps` to a div that acts as the drag separator.
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

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      setIsDragging(true);
      startPosRef.current =
        direction === "horizontal" ? e.clientX : e.clientY;
      startSizeRef.current = size;
    },
    [direction, size]
  );

  useEffect(() => {
    if (!isDragging) return;

    const onMouseMove = (e: MouseEvent) => {
      const currentPos =
        direction === "horizontal" ? e.clientX : e.clientY;
      const delta = currentPos - startPosRef.current;
      const newSize = reverse
        ? startSizeRef.current - delta
        : startSizeRef.current + delta;
      setSize(Math.max(minSize, Math.min(maxSize, newSize)));
    };

    const onMouseUp = () => {
      setIsDragging(false);
    };

    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);

    // Prevent text selection during drag
    document.body.style.userSelect = "none";
    document.body.style.cursor =
      direction === "horizontal" ? "col-resize" : "row-resize";

    return () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
    };
  }, [isDragging, direction, minSize, maxSize, reverse]);

  const handleProps = {
    onMouseDown,
    className: `resize-handle resize-handle-${direction}${isDragging ? " dragging" : ""}`,
    style: {
      cursor: direction === "horizontal" ? "col-resize" : "row-resize",
    } as React.CSSProperties,
  };

  return { size, setSize, handleProps, isDragging };
}
