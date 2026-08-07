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
import type { SessionTaskLink } from "./useSessionTaskLinks";
import { LaneTag } from "./LaneTag";

const SWIPE_ACTIONS_WIDTH = 128;

export interface SessionRowProps {
  session: SessionInfo;
  isActive: boolean;
  isBusy: boolean;
  hasActiveSubagent: boolean;
  isPinned: boolean;
  isRenaming: boolean;
  subagentCount: number;
  /** Originating kanban task/lane, when this session was launched from the board. */
  taskLink?: SessionTaskLink;
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
  taskLink,
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
          // A traversal stop for `useListNav`. Depth 1: sessions hang off the
          // project header, so `h` from here climbs to it.
          data-list-item=""
          data-list-key={session.id}
          data-list-depth={1}
        >
          {/* A dot, not a boxed glyph. Every row carried the same chat bubble
              in the same box, so the icon column cost 6 boxes and told the
              reader nothing; the dot spends the same space on the one thing
              that differs between rows — which runner owns the session. */}
          <span
            className={`sb-session-dot${isPinned ? " is-pinned" : ""}`}
            data-runner={session.runner || "unknown"}
            aria-hidden="true"
          >
            {isPinned && <Pin size={9} />}
          </span>
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
                <span className="sb-session-title" title={session.runner ? `${session.title || session.id} · ${session.runner}` : undefined}>
                  {session.title || session.id.slice(0, 12)}
                </span>
                <span className="sb-session-meta">
                  {/* The dot's colour is only decodable once you know the code,
                      so the engine is named outright next to it. */}
                  {session.runner && (
                    <span className="sb-runner-badge" data-runner={session.runner}>
                      {session.runner}
                    </span>
                  )}
                  {taskLink && <LaneTag link={taskLink} />}
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
                  <span className="sb-session-time" title="Last activity">
                    {formatTime(session.time.updated)}
                  </span>
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
