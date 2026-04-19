/**
 * PdfCanvasViewer — renders PDF pages onto <canvas> elements using pdfjs-dist.
 *
 * Supports page navigation and zoom. The rendered canvas is directly
 * capturable by markupCapture.ts (matched by `canvas:not(.markup-canvas)`).
 */
import { useState, useEffect, useRef, useCallback } from "react";
import { ChevronLeft, ChevronRight, ZoomIn, ZoomOut } from "lucide-react";
import * as pdfjsLib from "pdfjs-dist";
import type { PDFDocumentProxy } from "pdfjs-dist";

// Configure worker
pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
  "pdfjs-dist/build/pdf.worker.mjs",
  import.meta.url,
).href;

interface Props {
  url: string;
  alt?: string;
}

const ZOOM_STEP = 0.25;
const ZOOM_MIN = 0.5;
const ZOOM_MAX = 3;

export function PdfCanvasViewer({ url, alt }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [pdf, setPdf] = useState<PDFDocumentProxy | null>(null);
  const [page, setPage] = useState(1);
  const [numPages, setNumPages] = useState(0);
  const [zoom, setZoom] = useState(1);
  const [error, setError] = useState<string | null>(null);
  const renderTaskRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load document
  useEffect(() => {
    let cancelled = false;
    const task = pdfjsLib.getDocument(url);
    task.promise
      .then((doc) => {
        if (cancelled) { doc.destroy(); return; }
        setPdf(doc);
        setNumPages(doc.numPages);
        setPage(1);
        setError(null);
      })
      .catch((err) => {
        if (!cancelled) setError(err?.message ?? "Failed to load PDF");
      });
    return () => { cancelled = true; task.destroy(); };
  }, [url]);

  // Render current page
  const renderPage = useCallback(async () => {
    if (!pdf || !canvasRef.current || !containerRef.current) return;
    try {
      const pg = await pdf.getPage(page);
      const container = containerRef.current;
      const containerW = container.clientWidth;
      const unscaledViewport = pg.getViewport({ scale: 1 });
      const baseScale = containerW / unscaledViewport.width;
      const scale = baseScale * zoom;
      const viewport = pg.getViewport({ scale });
      const dpr = window.devicePixelRatio || 1;

      const canvas = canvasRef.current;
      canvas.width = viewport.width * dpr;
      canvas.height = viewport.height * dpr;
      canvas.style.width = `${viewport.width}px`;
      canvas.style.height = `${viewport.height}px`;

      const ctx = canvas.getContext("2d")!;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      await pg.render({ canvasContext: ctx, viewport, canvas } as any).promise;
    } catch (err: any) {
      console.warn("[PdfCanvasViewer] render failed:", err);
    }
  }, [pdf, page, zoom]);

  useEffect(() => {
    if (renderTaskRef.current) clearTimeout(renderTaskRef.current);
    renderTaskRef.current = setTimeout(renderPage, 30);
    return () => { if (renderTaskRef.current) clearTimeout(renderTaskRef.current); };
  }, [renderPage]);

  if (error) {
    return (
      <div className="file-preview file-preview-pdf">
        <span className="file-preview-label">{error}</span>
      </div>
    );
  }

  return (
    <div className="pdf-canvas-viewer" ref={containerRef}>
      <canvas ref={canvasRef} data-pdf-canvas aria-label={alt ?? "PDF page"} />
      {numPages > 0 && (
        <div className="pdf-canvas-controls">
          <button
            className="markup-tool-btn"
            disabled={page <= 1}
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            aria-label="Previous page"
          >
            <ChevronLeft size={15} />
          </button>
          <span className="pdf-canvas-page-label">
            {page} / {numPages}
          </span>
          <button
            className="markup-tool-btn"
            disabled={page >= numPages}
            onClick={() => setPage((p) => Math.min(numPages, p + 1))}
            aria-label="Next page"
          >
            <ChevronRight size={15} />
          </button>
          <span className="markup-toolbar-sep" />
          <button
            className="markup-tool-btn"
            disabled={zoom <= ZOOM_MIN}
            onClick={() => setZoom((z) => Math.max(ZOOM_MIN, z - ZOOM_STEP))}
            aria-label="Zoom out"
          >
            <ZoomOut size={15} />
          </button>
          <span className="pdf-canvas-page-label">{Math.round(zoom * 100)}%</span>
          <button
            className="markup-tool-btn"
            disabled={zoom >= ZOOM_MAX}
            onClick={() => setZoom((z) => Math.min(ZOOM_MAX, z + ZOOM_STEP))}
            aria-label="Zoom in"
          >
            <ZoomIn size={15} />
          </button>
        </div>
      )}
    </div>
  );
}
