/**
 * MobileFileEntries — SwipeFileEntry + MobileRenameOverlay components.
 * Extracted from MobileLayout to keep each file under 300 lines.
 */
import { useState, useRef, useEffect } from "react";
import { Folder, File, Trash2, Pencil, Download } from "lucide-react";
import type { FileEntry } from "../types";
import { formatSize } from "../types";
import { useSwipeReveal } from "../../hooks/useSwipeReveal";

// ── Swipe-to-reveal file entry (mobile) ─────────────────

export function SwipeFileEntry({ entry, onEntryClick, onDelete, onRename, onDownload, canDelete }: {
  entry: FileEntry;
  onEntryClick: (entry: FileEntry) => void;
  onDelete: () => void;
  onRename?: () => void;
  onDownload?: () => void;
  canDelete: boolean;
}) {
  const actionCount = (onRename ? 1 : 0) + (onDownload ? 1 : 0) + (canDelete ? 1 : 0);
  const actionsWidth = actionCount * 44;
  const swipe = useSwipeReveal({ actionsWidth });

  if (actionCount === 0) {
    return (
      <div className="code-editor-file-entry-row">
        <button className="code-editor-file-entry" onClick={() => onEntryClick(entry)}>
          {entry.is_dir ? <Folder size={14} className="file-icon folder-icon" /> : <File size={14} className="file-icon" />}
          <span className="file-name">{entry.name}</span>
          {!entry.is_dir && <span className="file-size">{formatSize(entry.size)}</span>}
        </button>
      </div>
    );
  }

  return (
    <div className={`${swipe.containerClass} swipe-row-explorer`} {...swipe.handlers}>
      <div className="swipe-row-actions">
        {onRename && (
          <button className="swipe-action-btn swipe-action-primary" title="Rename" onClick={() => { swipe.close(); onRename(); }}>
            <Pencil size={14} />
          </button>
        )}
        {onDownload && (
          <button className="swipe-action-btn swipe-action-success" title="Download" onClick={() => { swipe.close(); onDownload(); }}>
            <Download size={14} />
          </button>
        )}
        {canDelete && (
          <button className="swipe-action-btn swipe-action-danger" title="Delete" onClick={() => { swipe.close(); onDelete(); }}>
            <Trash2 size={14} />
          </button>
        )}
      </div>
      <div className="swipe-row-content" style={swipe.contentStyle}>
        <button className="code-editor-file-entry" onClick={() => onEntryClick(entry)}>
          {entry.is_dir ? <Folder size={14} className="file-icon folder-icon" /> : <File size={14} className="file-icon" />}
          <span className="file-name">{entry.name}</span>
          {!entry.is_dir && <span className="file-size">{formatSize(entry.size)}</span>}
        </button>
      </div>
    </div>
  );
}

// ── Mobile rename overlay ───────────────────────────────

export function MobileRenameOverlay({ entry, onSubmit, onCancel }: {
  entry: { name: string; isDir: boolean };
  onSubmit: (newName: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(entry.name);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus();
    if (!entry.isDir) {
      const dot = entry.name.lastIndexOf(".");
      if (dot > 0) el.setSelectionRange(0, dot);
      else el.select();
    } else {
      el.select();
    }
  }, [entry.name, entry.isDir]);

  const handleSubmit = () => {
    const trimmed = value.trim();
    if (trimmed && trimmed !== entry.name) onSubmit(trimmed);
    else onCancel();
  };

  return (
    <div className="mobile-inline-create">
      <Pencil size={14} className="file-icon" />
      <input
        ref={ref} className="explorer-inline-name-input" value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") handleSubmit(); if (e.key === "Escape") onCancel(); }}
        onBlur={handleSubmit}
      />
    </div>
  );
}
