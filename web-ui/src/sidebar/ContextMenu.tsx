import React, { useState, useCallback, useEffect } from "react";
import { Pin, Pencil, Trash2 } from "lucide-react";

// ── Types ────────────────────────────────────────────

export interface ContextMenuState {
  sessionId: string;
  sessionTitle: string;
  x: number;
  y: number;
  projectIdx: number;
}

// ── Helpers ──────────────────────────────────────────

function isMobile(): boolean {
  return window.innerWidth <= 768;
}

const MENU_W = 160;
const MENU_H = 140;
const PAD = 8;

function clampedPosition(x: number, y: number): [number, number] {
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  return [
    Math.max(PAD, Math.min(x, vw - MENU_W - PAD)),
    Math.max(PAD, Math.min(y, vh - MENU_H - PAD)),
  ];
}

// ── Hook ─────────────────────────────────────────────

export function useContextMenu() {
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);

  // Close on click outside or Escape
  useEffect(() => {
    if (!contextMenu) return;
    const handleClick = () => setContextMenu(null);
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setContextMenu(null);
    };
    document.addEventListener("click", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("click", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [contextMenu]);

  const handleContextMenu = useCallback(
    (
      e: React.MouseEvent,
      sessionId: string,
      sessionTitle: string,
      projectIdx: number,
    ) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({ sessionId, sessionTitle, x: e.clientX, y: e.clientY, projectIdx });
    },
    [],
  );

  return { contextMenu, setContextMenu, handleContextMenu } as const;
}

// ── Presentational component ─────────────────────────

interface SessionContextMenuProps {
  menu: ContextMenuState;
  isPinned: boolean;
  onPin: () => void;
  onRename: () => void;
  onDelete: () => void;
}

export function SessionContextMenu({
  menu,
  isPinned,
  onPin,
  onRename,
  onDelete,
}: SessionContextMenuProps) {
  const mobile = isMobile();
  const iconSz = mobile ? 16 : 12;

  const displayTitle = !menu.sessionTitle
    ? menu.sessionId.slice(0, 16)
    : menu.sessionTitle.length > 32
      ? menu.sessionTitle.slice(0, 29) + "..."
      : menu.sessionTitle;

  const items = (
    <>
      <button className="sb-context-item" onClick={onPin}>
        <Pin size={iconSz} />
        {isPinned ? "Unpin" : "Pin to Top"}
      </button>
      <button className="sb-context-item" onClick={onRename}>
        <Pencil size={iconSz} />
        Rename
      </button>
      <button className="sb-context-item sb-context-danger" onClick={onDelete}>
        <Trash2 size={iconSz} />
        Delete
      </button>
    </>
  );

  if (mobile) {
    // Mobile: bottom action-sheet with backdrop overlay.
    // Clicking the overlay (or Cancel) bubbles to the document click
    // listener in useContextMenu which closes the menu.
    return (
      <div className="sb-ctx-overlay">
        <div
          className="sb-context-menu sb-context-sheet"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="sb-ctx-sheet-title">{displayTitle}</div>
          {items}
          <button className="sb-context-item sb-ctx-cancel">
            Cancel
          </button>
        </div>
      </div>
    );
  }

  // Desktop: viewport-clamped fixed dropdown
  const [left, top] = clampedPosition(menu.x, menu.y);
  return (
    <div
      className="sb-context-menu"
      style={{ left, top }}
      onClick={(e) => e.stopPropagation()}
    >
      {items}
    </div>
  );
}
