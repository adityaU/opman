/**
 * ImagePreviewZoom — image preview with scroll-to-zoom, drag-to-pan, double-click-to-reset.
 * Matches Leptos image_preview.rs.
 */
import { useState, useCallback, useRef } from "react";

interface Props {
  url: string;
  alt: string;
}

export function ImagePreviewZoom({ url, alt }: Props) {
  const [scale, setScale] = useState(1);
  const [tx, setTx] = useState(0);
  const [ty, setTy] = useState(0);
  const dragging = useRef(false);
  const dragStart = useRef({ x: 0, y: 0, tx: 0, ty: 0 });

  const transform = `translate(${tx}px, ${ty}px) scale(${scale})`;
  const scalePct = `${Math.round(scale * 100)}%`;

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
    setScale((s) => Math.min(20, Math.max(0.1, s * factor)));
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    dragging.current = true;
    dragStart.current = { x: e.clientX, y: e.clientY, tx, ty };
  }, [tx, ty]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!dragging.current) return;
    const ds = dragStart.current;
    setTx(ds.tx + e.clientX - ds.x);
    setTy(ds.ty + e.clientY - ds.y);
  }, []);

  const handleMouseUp = useCallback(() => { dragging.current = false; }, []);

  const handleDblClick = useCallback(() => {
    setScale(1); setTx(0); setTy(0);
  }, []);

  const zoomIn  = useCallback(() => setScale((s) => Math.min(20, s * 1.25)), []);
  const zoomOut = useCallback(() => setScale((s) => Math.max(0.1, s / 1.25)), []);
  const reset   = useCallback(() => { setScale(1); setTx(0); setTy(0); }, []);

  return (
    <div className="file-preview file-preview-image-zoom">
      <div className="image-zoom-toolbar">
        <button className="image-zoom-btn" onClick={zoomOut} title="Zoom out">&minus;</button>
        <span className="image-zoom-level">{scalePct}</span>
        <button className="image-zoom-btn" onClick={zoomIn} title="Zoom in">+</button>
        <button className="image-zoom-btn image-zoom-reset" onClick={reset} title="Reset (or double-click)">&#8634;</button>
      </div>
      <div
        className="image-zoom-canvas"
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onDoubleClick={handleDblClick}
        style={{ cursor: dragging.current ? "grabbing" : "grab" }}
      >
        <img
          src={url}
          alt={alt}
          draggable={false}
          style={{ transform, transformOrigin: "center center" }}
        />
      </div>
      <div className="image-zoom-hint">Scroll to zoom &bull; Drag to pan &bull; Double-click to reset</div>
    </div>
  );
}
