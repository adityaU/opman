import React from "react";
import type { SessionInfo } from "../api";
import {
  MessageSquare,
  Pin,
  Zap,
  MoreHorizontal,
  Pencil,
  Trash2,
} from "lucide-react";
import { useSwipeReveal } from "../hooks/useSwipeReveal";
import { formatTime } from "./formatTime";

const SWIPE_ACTIONS_WIDTH = 128;

export interface SessionRowProps {
  session: SessionInfo;
  isActive: boolean;
  isBusy: boolean;
  hasActiveSubagent: boolean;
  isPinned: boolean;
  isRenaming: boolean;
  subagentCount: number;
  renameValue: string;
  renameLoading: boolean;
  renameInputRef: React.RefObject<HTMLInputElement>;
  onSelect: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
  onToggleSubagents: () => void;
  onRenameValueChange: (value: string) => void;
  onRenameKeyDown: (e: React.KeyboardEvent) => void;
  onRenameSubmit: () => void;
  onRenameCancel: () => void;
  onSwipePin: () => void;
  onSwipeRename: () => void;
  onSwipeDelete: () => void;
}

export const SessionRow = React.memo(function SessionRow({
  session,
  isActive,
  isBusy,
  hasActiveSubagent,
  isPinned,
  isRenaming,
  subagentCount,
  renameValue,
  renameLoading,
  renameInputRef,
  onSelect,
  onContextMenu,
  onToggleSubagents,
  onRenameValueChange,
  onRenameKeyDown,
  onRenameCancel,
  onSwipePin,
  onSwipeRename,
  onSwipeDelete,
}: SessionRowProps) {
  const swipe = useSwipeReveal({ actionsWidth: SWIPE_ACTIONS_WIDTH });
  const busy = isBusy || hasActiveSubagent;

  return (
    <div className={swipe.containerClass} {...swipe.handlers}>
      {/* Swipe action tray */}
      <div className="swipe-row-actions">
        <button
          className="swipe-action-btn swipe-action-primary"
          title="Pin / Unpin"
          onClick={(e) => { e.stopPropagation(); onSwipePin(); swipe.close(); }}
        >
          <Pin size={14} />
        </button>
        <button
          className="swipe-action-btn"
          title="Rename"
          onClick={(e) => { e.stopPropagation(); onSwipeRename(); swipe.close(); }}
        >
          <Pencil size={14} />
        </button>
        <button
          className="swipe-action-btn swipe-action-danger"
          title="Delete"
          onClick={(e) => { e.stopPropagation(); onSwipeDelete(); swipe.close(); }}
        >
          <Trash2 size={14} />
        </button>
      </div>

      {/* Front content layer */}
      <div className="swipe-row-content" style={swipe.contentStyle}>
        <button
          className={`sb-session${isActive ? " active" : ""}${busy ? " busy" : ""}`}
          onClick={() => { if (!isRenaming) onSelect(); }}
          onContextMenu={onContextMenu}
        >
          <div className="sb-session-icon">
            {isPinned ? <Pin size={12} className="sb-pin-icon" /> : <MessageSquare size={14} />}
          </div>
          <div className="sb-session-info">
            {isRenaming ? (
              <input
                ref={renameInputRef}
                className="sb-rename-input"
                type="text"
                value={renameValue}
                onChange={(e) => onRenameValueChange(e.target.value)}
                onKeyDown={onRenameKeyDown}
                onBlur={() => {
                  setTimeout(() => { if (!renameLoading) onRenameCancel(); }, 150);
                }}
                onClick={(e) => e.stopPropagation()}
                disabled={renameLoading}
              />
            ) : (
              <>
                <span className="sb-session-title">
                  {session.title || session.id.slice(0, 12)}
                </span>
                <span className="sb-session-meta">
                  {formatTime(session.time.updated)}
                  {subagentCount > 0 && (
                    <span
                      className="sb-subagent-badge"
                      onClick={(e) => { e.stopPropagation(); onToggleSubagents(); }}
                      title={`${subagentCount} subagent${subagentCount > 1 ? "s" : ""}`}
                    >
                      <Zap size={8} />
                      {subagentCount}
                    </span>
                  )}
                </span>
              </>
            )}
          </div>
          {busy && <span className="sb-busy-indicator" />}
          {!isRenaming && (
            <span className="sb-session-actions" onClick={onContextMenu}>
              <MoreHorizontal size={14} />
            </span>
          )}
        </button>
      </div>
    </div>
  );
});
