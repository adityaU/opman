/**
 * MarkupOverlay — annotation overlay for file previews.
 *
 * Renders a full-size canvas on top of the preview element,
 * with a floating toolbar for pen / rectangle / text / eraser / color / undo.
 * On save, flattens the underlying preview + canvas into a PNG and copies to clipboard.
 */
import { useState, useRef, useCallback, useEffect } from "react";
import { capturePreview } from "./markupCapture";
import { MarkupToolbar } from "./MarkupToolbar";

// ── Types ───────────────────────────────────────────────

type Tool = "pen" | "rect" | "text" | "eraser";

interface Stroke {
  tool: "pen" | "eraser";
  points: { x: number; y: number }[];
  color: string;
  width: number;
}

interface Rect {
  tool: "rect";
  x: number; y: number; w: number; h: number;
  color: string;
  width: number;
}

interface TextAnnotation {
  tool: "text";
  x: number; y: number;
  text: string;
  color: string;
  fontSize: number;
}

type MarkupAction = Stroke | Rect | TextAnnotation;

const DEFAULT_COLOR = "#ff3b30";
const PEN_WIDTH = 3;
const ERASER_WIDTH = 20;
const FONT_SIZE = 18;

// ── Component ───────────────────────────────────────────

interface Props {
  previewRef: React.RefObject<HTMLElement | null>;
  onClose: () => void;
}

export function MarkupOverlay({ previewRef, onClose }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [tool, setTool] = useState<Tool>("pen");
  const [color, setColor] = useState(DEFAULT_COLOR);
  const [penWidth, setPenWidth] = useState(PEN_WIDTH);
  const [actions, setActions] = useState<MarkupAction[]>([]);
  const [drawing, setDrawing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const currentStroke = useRef<{ x: number; y: number }[]>([]);
  const rectStart = useRef<{ x: number; y: number } | null>(null);
  const rectCurrent = useRef<{ x: number; y: number } | null>(null);

  // ── Canvas sizing ────────────────────────────────────

  const syncSize = useCallback(() => {
    const canvas = canvasRef.current;
    const parent = canvas?.parentElement;
    if (!canvas || !parent) return;
    const r = parent.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    canvas.width = r.width * dpr;
    canvas.height = r.height * dpr;
    canvas.style.width = `${r.width}px`;
    canvas.style.height = `${r.height}px`;
    const ctx = canvas.getContext("2d");
    if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    redraw();
  }, []);

  useEffect(() => {
    syncSize();
    const ro = new ResizeObserver(syncSize);
    const parent = canvasRef.current?.parentElement;
    if (parent) ro.observe(parent);
    return () => ro.disconnect();
  }, [syncSize]);

  useEffect(() => { redraw(); }, [actions]);

  // ── Drawing helpers ──────────────────────────────────

  function redraw() {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    for (const a of actions) renderAction(ctx, a);
  }

  function renderAction(ctx: CanvasRenderingContext2D, a: MarkupAction) {
    ctx.save();
    if (a.tool === "pen" || a.tool === "eraser") {
      ctx.globalCompositeOperation = a.tool === "eraser" ? "destination-out" : "source-over";
      ctx.strokeStyle = a.color;
      ctx.lineWidth = a.width;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.beginPath();
      a.points.forEach((p, i) => (i === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y)));
      ctx.stroke();
    } else if (a.tool === "rect") {
      ctx.strokeStyle = a.color;
      ctx.lineWidth = a.width;
      ctx.strokeRect(a.x, a.y, a.w, a.h);
    } else if (a.tool === "text") {
      ctx.fillStyle = a.color;
      ctx.font = `${a.fontSize}px var(--font-mono, monospace)`;
      ctx.textBaseline = "top";
      ctx.fillText(a.text, a.x, a.y);
    }
    ctx.restore();
  }

  function canvasXY(e: React.PointerEvent) {
    const r = canvasRef.current!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  // ── Pointer events ──────────────────────────────────

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const pos = canvasXY(e);
    if (tool === "text") {
      const text = prompt("Enter annotation text:");
      if (!text) return;
      setActions((prev) => [...prev, { tool: "text", x: pos.x, y: pos.y, text, color, fontSize: FONT_SIZE }]);
      return;
    }
    setDrawing(true);
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    if (tool === "pen" || tool === "eraser") currentStroke.current = [pos];
    else if (tool === "rect") { rectStart.current = pos; rectCurrent.current = pos; }
  }, [tool, color]);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!drawing) return;
    const pos = canvasXY(e);
    if (tool === "pen" || tool === "eraser") {
      currentStroke.current.push(pos);
      const ctx = canvasRef.current?.getContext("2d");
      const pts = currentStroke.current;
      if (!ctx || pts.length < 2) return;
      ctx.save();
      ctx.globalCompositeOperation = tool === "eraser" ? "destination-out" : "source-over";
      ctx.strokeStyle = color;
      ctx.lineWidth = tool === "eraser" ? ERASER_WIDTH : penWidth;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.beginPath();
      ctx.moveTo(pts[pts.length - 2].x, pts[pts.length - 2].y);
      ctx.lineTo(pos.x, pos.y);
      ctx.stroke();
      ctx.restore();
    } else if (tool === "rect") {
      rectCurrent.current = pos;
      redraw();
      const ctx = canvasRef.current?.getContext("2d");
      const s = rectStart.current;
      if (!ctx || !s) return;
      ctx.save();
      ctx.strokeStyle = color;
      ctx.lineWidth = penWidth;
      ctx.strokeRect(s.x, s.y, pos.x - s.x, pos.y - s.y);
      ctx.restore();
    }
  }, [drawing, tool, color, penWidth]);

  const onPointerUp = useCallback(() => {
    if (!drawing) return;
    setDrawing(false);
    if (tool === "pen" || tool === "eraser") {
      const pts = [...currentStroke.current];
      if (pts.length > 0) {
        setActions((prev) => [...prev, { tool, points: pts, color, width: tool === "eraser" ? ERASER_WIDTH : penWidth }]);
      }
      currentStroke.current = [];
    } else if (tool === "rect" && rectStart.current && rectCurrent.current) {
      const s = rectStart.current, c = rectCurrent.current;
      if (Math.abs(c.x - s.x) > 2 || Math.abs(c.y - s.y) > 2) {
        setActions((prev) => [...prev, { tool: "rect", x: s.x, y: s.y, w: c.x - s.x, h: c.y - s.y, color, width: penWidth }]);
      }
      rectStart.current = null;
      rectCurrent.current = null;
    }
  }, [drawing, tool, color, penWidth]);

  const undo = useCallback(() => setActions((prev) => prev.slice(0, -1)), []);

  // ── Save to clipboard ──────────────────────────────

  const saveToClipboard = useCallback(async () => {
    const preview = previewRef.current;
    const canvas = canvasRef.current;
    if (!preview || !canvas) return;
    setSaving(true);
    try {
      const blob = await capturePreview(preview, canvas);
      if (blob) {
        await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
        setSaved(true);
        setTimeout(() => onClose(), 800);
      }
    } catch (err) {
      console.error("[Markup] Failed to copy to clipboard:", err);
    } finally {
      setSaving(false);
    }
  }, [previewRef, onClose]);

  // ── Keyboard shortcuts ──────────────────────────────

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") { onClose(); return; }
      if ((e.metaKey || e.ctrlKey) && e.key === "z") { e.preventDefault(); undo(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose, undo]);

  // ── Render ──────────────────────────────────────────

  return (
    <div className="markup-overlay">
      <canvas
        ref={canvasRef}
        className="markup-canvas"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        style={{ cursor: tool === "eraser" ? "cell" : "crosshair" }}
      />
      <MarkupToolbar
        tool={tool} setTool={setTool}
        color={color} setColor={setColor}
        penWidth={penWidth} setPenWidth={setPenWidth}
        canUndo={actions.length > 0} onUndo={undo}
        saving={saving} saved={saved} canSave={actions.length > 0}
        onSave={saveToClipboard} onClose={onClose}
      />
    </div>
  );
}
