import React, { useMemo } from "react";
import type { ProjectInfo } from "../api";
import {
  Layers,
  Clock3,
  Pin,
  MessageCircle,
  X,
  XCircle,
  Pencil,
} from "lucide-react";
import { useSwipeReveal } from "../hooks/useSwipeReveal";
import { formatTime, toMs } from "./formatTime";
import type { SessionTaskLink } from "./useSessionTaskLinks";
import { LaneTag } from "./LaneTag";
import { isOpenSessionFresh } from "./openSessions";

// ── Types ────────────────────────────────────────────

/** Flat row derived from scanning all projects. */
interface OpenEntry {
  sid: string;
  title: string;
  projectName: string;
  projectIdx: number;
  updated: number;
  runner?: string;
}

interface OpenGroup {
  label: string;
  entries: OpenEntry[];
}

export interface OpenSessionsSectionProps {
  projects: ProjectInfo[];
  openSessions: Set<string>;
  activeSessionId: string | null;
  isSessionBusy: (sid: string) => boolean;
  busyKey: string;
  pinnedSessions: Set<string>;
  onSelectSession: (sessionId: string, projectIdx: number) => void;
  onRemoveOpen: (sessionId: string) => void;
  onTogglePin: (sessionId: string) => void;
  onStartRename: (sessionId: string, title: string) => void;
  onDeleteSession: (sessionId: string, title: string) => void;
  onContextMenu: (
    e: React.MouseEvent,
    sessionId: string,
    sessionTitle: string,
    projectIdx: number,
  ) => void;
  /** session_id → originating kanban task/lane (active project only). */
  sessionTaskLinks?: Map<string, SessionTaskLink>;
}

// ── Component ────────────────────────────────────────

export function OpenSessionsSection({
  projects,
  openSessions,
  activeSessionId,
  isSessionBusy,
  busyKey,
  pinnedSessions,
  onSelectSession,
  onRemoveOpen,
  onTogglePin,
  onStartRename,
  onDeleteSession,
  onContextMenu,
  sessionTaskLinks,
}: OpenSessionsSectionProps) {
  const entries = useMemo(() => {
    const out: OpenEntry[] = [];
    for (const p of projects) {
      for (const s of p.sessions) {
        if (!openSessions.has(s.id)) continue;
        if (!isOpenSessionFresh(s.time.updated)) continue;
        if (s.parentID && s.parentID !== "") continue;
        // Sessions launched from a kanban task live under their task group in
        // the project tree — keep them out of the plain Open Sessions list.
        if (sessionTaskLinks?.has(s.id)) continue;
        out.push({
          sid: s.id,
          title: s.title || s.id.slice(0, 12),
          projectName: p.name,
          projectIdx: p.index,
          updated: s.time.updated,
          runner: s.runner,
        });
      }
    }
    out.sort((a, b) => b.updated - a.updated);
    return out;
  }, [projects, openSessions, sessionTaskLinks]);

  if (entries.length === 0) return null;

  const groups = entries.reduce<OpenGroup[]>((result, entry) => {
    const label = Date.now() - toMs(entry.updated) < 24 * 60 * 60 * 1000 ? "Today" : "Yesterday";
    const group = result.find((item) => item.label === label);
    if (group) group.entries.push(entry);
    else result.push({ label, entries: [entry] });
    return result;
  }, []);

  return (
    <div className="sb-open-sessions">
      <div className="sb-open-header">
        <span className="sb-open-header-icon"><Layers size={12} /></span>
        <span className="sb-open-header-copy">
          <span>Open Sessions</span>
        </span>
        <span className="sb-open-count">{entries.length}</span>
      </div>
      {groups.map((group) => (
        <div className="sb-open-group" key={group.label}>
          <div className="sb-open-group-label"><Clock3 size={10} />{group.label}</div>
          {group.entries.map((e) => (
            <OpenSessionRow
              key={e.sid}
              entry={e}
              isActive={e.sid === activeSessionId}
              isBusy={isSessionBusy(e.sid)}
              isPinned={pinnedSessions.has(e.sid)}
              taskLink={sessionTaskLinks?.get(e.sid)}
              onSelect={() => onSelectSession(e.sid, e.projectIdx)}
              onRemove={() => onRemoveOpen(e.sid)}
              onTogglePin={() => onTogglePin(e.sid)}
              onStartRename={() => onStartRename(e.sid, e.title)}
              onDeleteSession={() => onDeleteSession(e.sid, e.title)}
              onContextMenu={onContextMenu}
            />
          ))}
        </div>
      ))}
    </div>
  );
}

// ── Single row ───────────────────────────────────────

const SWIPE_ACTIONS_WIDTH = 128;

interface OpenSessionRowProps {
  entry: OpenEntry;
  isActive: boolean;
  isBusy: boolean;
  isPinned: boolean;
  taskLink?: SessionTaskLink;
  onSelect: () => void;
  onRemove: () => void;
  onTogglePin: () => void;
  onStartRename: () => void;
  onDeleteSession: () => void;
  onContextMenu: (
    e: React.MouseEvent,
    sessionId: string,
    sessionTitle: string,
    projectIdx: number,
  ) => void;
}

const OpenSessionRow = React.memo(function OpenSessionRow({
  entry,
  isActive,
  isBusy,
  isPinned,
  taskLink,
  onSelect,
  onRemove,
  onTogglePin,
  onStartRename,
  onDeleteSession,
  onContextMenu,
}: OpenSessionRowProps) {
  const swipe = useSwipeReveal({ actionsWidth: SWIPE_ACTIONS_WIDTH });

  const handleCtx = (e: React.MouseEvent) => {
    onContextMenu(e, entry.sid, entry.title, entry.projectIdx);
  };

  return (
    <div className={swipe.containerClass} {...swipe.handlers}>
      {/* Swipe action tray (behind content) */}
      <div className="swipe-row-actions">
        <button
          className="swipe-action-btn swipe-action-primary"
          title="Pin / Unpin"
          onClick={(e) => { e.stopPropagation(); onTogglePin(); swipe.close(); }}
        >
          <Pin size={14} />
        </button>
        <button
          className="swipe-action-btn"
          title="Rename"
          onClick={(e) => { e.stopPropagation(); onStartRename(); swipe.close(); }}
        >
          <Pencil size={14} />
        </button>
        <button
          className="swipe-action-btn swipe-action-danger"
          title="Close Session"
          onClick={(e) => { e.stopPropagation(); onRemove(); swipe.close(); }}
        >
          <XCircle size={14} />
        </button>
      </div>

      {/* Front content layer */}
      <div className="swipe-row-content" style={swipe.contentStyle}>
        <button
          className={`sb-session sb-open-row${isActive ? " active" : ""}${isBusy ? " busy" : ""}`}
          onClick={onSelect}
          onContextMenu={handleCtx}
          data-list-item=""
          data-list-key={entry.sid}
          data-list-depth={0}
        >
          <div className="sb-session-icon">
            {isPinned ? <Pin size={12} className="sb-pin-icon" /> : <MessageCircle size={14} />}
          </div>
          <div className="sb-session-info">
            <span className="sb-session-title" title={entry.title}>{entry.title}</span>
            <span className="sb-session-meta">
              <span className="sb-open-project-tag">{entry.projectName}</span>
              {entry.runner && (
                <span className="sb-runner-badge" data-runner={entry.runner}>{entry.runner}</span>
              )}
              {taskLink && <LaneTag link={taskLink} />}
              <span className="sb-session-time" title="Last activity">
                {formatTime(entry.updated)}
              </span>
            </span>
          </div>
          {isBusy && <span className="sb-busy-indicator" />}
          <span
            className="sb-open-remove"
            title="Remove from Open Sessions"
            onClick={(e) => { e.stopPropagation(); onRemove(); }}
          >
            <X size={10} />
          </span>
        </button>
      </div>
    </div>
  );
});
