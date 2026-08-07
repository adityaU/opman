/**
 * MobileSheets — the folder-actions menu, the name prompt, and the delete
 * confirmation, all as bottom sheets.
 *
 * The old versions were a dropdown pinned under a 14px button and two inline
 * strips wedged into the list. On a phone that means a menu near the top of the
 * screen your thumb cannot comfortably reach, and a rename field that shares a
 * row with the list it is editing. Sheets rise from the bottom edge, put their
 * controls where the hand already is, and are large enough to hit.
 */
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { FilePlus, FolderPlus, Upload, RefreshCw, AlertTriangle } from "lucide-react";

interface SheetProps {
  title: string;
  children: React.ReactNode;
  onClose: () => void;
}

function Sheet({ title, children, onClose }: SheetProps) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  return createPortal(
    <div className="xplm-sheet-scrim" role="presentation" onClick={onClose}>
      <div
        className="xplm-sheet"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="xplm-sheet-grip" aria-hidden="true" />
        <div className="xplm-sheet-title">{title}</div>
        {children}
      </div>
    </div>,
    document.body,
  );
}

// ── Folder actions ──────────────────────────────────────

export interface FolderAction {
  key: string;
  label: string;
  icon: React.ReactNode;
  run: () => void;
}

export function buildFolderActions(handlers: {
  onNewFile?: () => void;
  onNewFolder?: () => void;
  onUpload?: () => void;
  onReload?: () => void;
}): FolderAction[] {
  const actions: FolderAction[] = [];
  if (handlers.onNewFile) {
    actions.push({ key: "file", label: "New file", icon: <FilePlus size={17} />, run: handlers.onNewFile });
  }
  if (handlers.onNewFolder) {
    actions.push({ key: "dir", label: "New folder", icon: <FolderPlus size={17} />, run: handlers.onNewFolder });
  }
  if (handlers.onUpload) {
    actions.push({ key: "upload", label: "Upload files", icon: <Upload size={17} />, run: handlers.onUpload });
  }
  if (handlers.onReload) {
    actions.push({ key: "reload", label: "Reload", icon: <RefreshCw size={17} />, run: handlers.onReload });
  }
  return actions;
}

export function ActionsSheet({ folder, actions, onClose }: {
  folder: string; actions: FolderAction[]; onClose: () => void;
}) {
  return (
    <Sheet title={folder} onClose={onClose}>
      <div className="xplm-sheet-list">
        {actions.map((action) => (
          <button
            key={action.key}
            type="button"
            className="xplm-sheet-item"
            onClick={() => { action.run(); onClose(); }}
          >
            {action.icon}
            <span>{action.label}</span>
          </button>
        ))}
      </div>
    </Sheet>
  );
}

// ── Name prompt (create + rename) ───────────────────────

export function NameSheet({ title, initial, placeholder, selectStem, confirmLabel, onSubmit, onClose }: {
  title: string;
  initial: string;
  placeholder?: string;
  selectStem?: boolean;
  confirmLabel: string;
  onSubmit: (name: string) => void;
  onClose: () => void;
}) {
  const [value, setValue] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const input = ref.current;
    if (!input) return;
    input.focus();
    if (!initial) return;
    const dot = selectStem ? initial.lastIndexOf(".") : -1;
    if (dot > 0) input.setSelectionRange(0, dot);
    else input.select();
  }, [initial, selectStem]);

  const submit = () => {
    const trimmed = value.trim();
    if (!trimmed) return;
    onSubmit(trimmed);
    onClose();
  };

  return (
    <Sheet title={title} onClose={onClose}>
      <input
        ref={ref}
        className="xplm-sheet-input"
        value={value}
        placeholder={placeholder}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => { if (event.key === "Enter") submit(); }}
      />
      <div className="xplm-sheet-buttons">
        <button type="button" className="xplm-sheet-cancel" onClick={onClose}>Cancel</button>
        <button
          type="button"
          className="xplm-sheet-confirm"
          disabled={!value.trim()}
          onClick={submit}
        >
          {confirmLabel}
        </button>
      </div>
    </Sheet>
  );
}

// ── Delete confirmation ─────────────────────────────────

export function DeleteSheet({ name, isDir, onConfirm, onClose }: {
  name: string; isDir: boolean; onConfirm: () => void; onClose: () => void;
}) {
  return (
    <Sheet title={`Delete ${isDir ? "folder" : "file"}`} onClose={onClose}>
      <div className="xplm-sheet-warning">
        <AlertTriangle size={18} />
        <span>
          <strong>{name}</strong>
          {isDir ? " and everything inside it will be deleted." : " will be deleted."}
          {" This cannot be undone."}
        </span>
      </div>
      <div className="xplm-sheet-buttons">
        <button type="button" className="xplm-sheet-cancel" onClick={onClose}>Keep</button>
        <button
          type="button"
          className="xplm-sheet-confirm is-danger"
          onClick={() => { onConfirm(); onClose(); }}
        >
          Delete
        </button>
      </div>
    </Sheet>
  );
}
