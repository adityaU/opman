/**
 * MarkupToolbar — floating toolbar for the markup annotation overlay.
 */
import {
  Pen, Square, Type, Eraser, Undo2, Copy, X,
  Minus, Plus,
} from "lucide-react";
import { KeyHint } from "../../keybindings/hint/KeyHint";

type Tool = "pen" | "rect" | "text" | "eraser";

const COLORS = ["#ff3b30", "#ff9500", "#ffcc00", "#34c759", "#007aff", "#af52de", "#ffffff", "#000000"];

interface Props {
  tool: Tool;
  setTool: (t: Tool) => void;
  color: string;
  setColor: (c: string) => void;
  penWidth: number;
  setPenWidth: React.Dispatch<React.SetStateAction<number>>;
  canUndo: boolean;
  onUndo: () => void;
  saving: boolean;
  saved: boolean;
  canSave: boolean;
  onSave: () => void;
  onClose: () => void;
}

export function MarkupToolbar({
  tool, setTool, color, setColor, penWidth, setPenWidth,
  canUndo, onUndo, saving, saved, canSave, onSave, onClose,
}: Props) {
  return (
    <div className="markup-toolbar">
      <div className="markup-toolbar-group">
        <button className={`markup-tool-btn ${tool === "pen" ? "active" : ""}`} onClick={() => setTool("pen")} title="Pen (draw)">
          <Pen size={15} />
        </button>
        <button className={`markup-tool-btn ${tool === "rect" ? "active" : ""}`} onClick={() => setTool("rect")} title="Rectangle">
          <Square size={15} />
        </button>
        <button className={`markup-tool-btn ${tool === "text" ? "active" : ""}`} onClick={() => setTool("text")} title="Text (click to place)">
          <Type size={15} />
        </button>
        <button className={`markup-tool-btn ${tool === "eraser" ? "active" : ""}`} onClick={() => setTool("eraser")} title="Eraser">
          <Eraser size={15} />
        </button>
      </div>

      <div className="markup-toolbar-sep" />

      <div className="markup-toolbar-group markup-colors">
        {COLORS.map((c) => (
          <button
            key={c}
            className={`markup-color-btn ${c === color ? "active" : ""}`}
            style={{ backgroundColor: c }}
            onClick={() => setColor(c)}
            title={c}
          />
        ))}
      </div>

      <div className="markup-toolbar-sep" />

      <div className="markup-toolbar-group markup-width-group">
        <button className="markup-tool-btn" onClick={() => setPenWidth((w) => Math.max(1, w - 1))} title="Thinner">
          <Minus size={13} />
        </button>
        <span className="markup-width-label">{penWidth}px</span>
        <button className="markup-tool-btn" onClick={() => setPenWidth((w) => Math.min(20, w + 1))} title="Thicker">
          <Plus size={13} />
        </button>
      </div>

      <div className="markup-toolbar-sep" />

      <div className="markup-toolbar-group">
        <KeyHint label="Undo stroke" command="markup.undo">
          <button className="markup-tool-btn" onClick={onUndo} disabled={!canUndo} aria-label="Undo stroke">
            <Undo2 size={15} />
          </button>
        </KeyHint>
      </div>

      <span className="markup-toolbar-spacer" />

      <div className="markup-toolbar-group markup-toolbar-actions">
        <button className="markup-action-btn markup-save" onClick={onSave} disabled={saving || !canSave} title="Copy to clipboard">
          {saved ? "Copied!" : saving ? "Saving..." : <><Copy size={13} /> Copy</>}
        </button>
        <KeyHint label="Cancel" command="markup.cancel">
          <button className="markup-action-btn markup-cancel" onClick={onClose}>
            <X size={13} /> Cancel
          </button>
        </KeyHint>
      </div>
    </div>
  );
}
