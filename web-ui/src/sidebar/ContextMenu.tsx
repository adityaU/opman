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
  return (
    <div
      className="sb-context-menu"
      style={{ left: menu.x, top: menu.y }}
      onClick={(e) => e.stopPropagation()}
    >
      <button className="sb-context-item" onClick={onPin}>
        <Pin size={12} />
        {isPinned ? "Unpin" : "Pin to Top"}
      </button>
      <button className="sb-context-item" onClick={onRename}>
        <Pencil size={12} />
        Rename
      </button>
      <button className="sb-context-item sb-context-danger" onClick={onDelete}>
        <Trash2 size={12} />
        Delete
      </button>
    </div>
  );
}
