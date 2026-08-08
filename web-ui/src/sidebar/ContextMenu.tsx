import React, { useState, useCallback, useEffect } from "react";
import { Pin, Pencil, Trash2, XCircle, SquareKanban } from "lucide-react";
import type { SessionTaskLink } from "./useSessionTaskLinks";
import { isMobileViewport } from "../hooks/useIsMobile";
import type { CommandId } from "../keybindings/types";
import { useChordLabeller } from "../keybindings/useChord";

// ── Types ────────────────────────────────────────────

export interface ContextMenuState {
  sessionId: string;
  sessionTitle: string;
  x: number;
  y: number;
  projectIdx: number;
}

// ── Helpers ──────────────────────────────────────────

const MENU_W = 180;
const MENU_H = 200;
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
  isOpen: boolean;
  /** Originating kanban task/lane, when this session was launched from the board. */
  taskLink?: SessionTaskLink;
  onPin: () => void;
  onRename: () => void;
  onDelete: () => void;
  onRemoveOpen: () => void;
  /** Open the originating kanban task's editor (only meaningful when taskLink is set). */
  onOpenTask?: () => void;
}

export function SessionContextMenu({
  menu,
  isPinned,
  isOpen,
  taskLink,
  onPin,
  onRename,
  onDelete,
  onRemoveOpen,
  onOpenTask,
}: SessionContextMenuProps) {
  const mobile = isMobileViewport();
  const iconSz = mobile ? 16 : 12;
  const chordFor = useChordLabeller();

  /**
   * The chord for a row, on desktop only. A touch sheet has no keyboard to
   * teach, and the cap would only crowd the label.
   */
  const key = (command: CommandId) => {
    const chord = mobile ? undefined : chordFor(command);
    return chord ? <kbd className="sb-context-kbd">{chord}</kbd> : null;
  };

  const displayTitle = !menu.sessionTitle
    ? menu.sessionId.slice(0, 16)
    : menu.sessionTitle.length > 32
      ? menu.sessionTitle.slice(0, 29) + "..."
      : menu.sessionTitle;

  const items = (
    <>
      {taskLink && onOpenTask && (
        <button className="sb-context-item" onClick={onOpenTask} title={`Lane: ${taskLink.laneName}`}>
          <SquareKanban size={iconSz} />
          Go to Kanban task
        </button>
      )}
      <button className="sb-context-item" onClick={onPin}>
        <Pin size={iconSz} />
        {isPinned ? "Unpin" : "Pin to Top"}
        {key("session.togglePin")}
      </button>
      <button className="sb-context-item" onClick={onRename}>
        <Pencil size={iconSz} />
        Rename
        {key("session.rename")}
      </button>
      {isOpen && (
        <button className="sb-context-item" onClick={onRemoveOpen}>
          <XCircle size={iconSz} />
          Close Session
          {key("session.close")}
        </button>
      )}
      <button className="sb-context-item sb-context-danger" onClick={onDelete}>
        <Trash2 size={iconSz} />
        Delete
        {key("session.delete")}
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
