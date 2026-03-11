import React, { useMemo } from "react";
import type { ProjectInfo, SessionInfo } from "../api";
import {
  ChevronDown,
  ChevronRight,
  MessageSquare,
  Pin,
  Zap,
  MoreHorizontal,
  GitBranch,
} from "lucide-react";
import { formatTime } from "./formatTime";

const MAX_VISIBLE_SESSIONS = 8;

export interface ProjectNodeProps {
  project: ProjectInfo;
  index: number;
  isActiveProject: boolean;
  isExpanded: boolean;
  activeSessionId: string | null;
  busySessions: Set<string>;
  expandedSubagents: string | null;
  showMore: boolean;
  searchQuery: string;
  onToggleExpand: () => void;
  onSelectSession: (sessionId: string, projectIdx: number) => void;
  onNewSession: () => void;
  onToggleSubagents: (sessionId: string) => void;
  onShowMore: () => void;
  onContextMenu: (
    e: React.MouseEvent,
    sessionId: string,
    sessionTitle: string,
    projectIdx: number
  ) => void;
  renameTarget: { sessionId: string; currentTitle: string } | null;
  renameValue: string;
  renameLoading: boolean;
  renameInputRef: React.RefObject<HTMLInputElement>;
  onRenameValueChange: (value: string) => void;
  onRenameKeyDown: (e: React.KeyboardEvent) => void;
  onRenameSubmit: () => void;
  onRenameCancel: () => void;
  pinnedSessions: Set<string>;
}

export function ProjectNode({
  project,
  index,
  isActiveProject,
  isExpanded,
  activeSessionId,
  busySessions,
  expandedSubagents,
  showMore,
  searchQuery,
  onToggleExpand,
  onSelectSession,
  onNewSession,
  onToggleSubagents,
  onShowMore,
  onContextMenu,
  renameTarget,
  renameValue,
  renameLoading,
  renameInputRef,
  onRenameValueChange,
  onRenameKeyDown,
  onRenameSubmit,
  onRenameCancel,
  pinnedSessions,
}: ProjectNodeProps) {
  const { parentSessions, childrenMap, hasActive } = useMemo(() => {
    const parents: SessionInfo[] = [];
    const children: Map<string, SessionInfo[]> = new Map();

    for (const s of project.sessions) {
      if (!s.parentID || s.parentID === "") {
        parents.push(s);
      } else {
        const list = children.get(s.parentID) || [];
        list.push(s);
        children.set(s.parentID, list);
      }
    }

    // Sort: pinned first (preserving recency within each group)
    parents.sort((a, b) => {
      const ap = pinnedSessions.has(a.id) ? 1 : 0;
      const bp = pinnedSessions.has(b.id) ? 1 : 0;
      if (ap !== bp) return bp - ap; // pinned first
      return b.time.updated - a.time.updated;
    });

    let active = false;
    for (const s of project.sessions) {
      if (busySessions.has(s.id)) {
        active = true;
        break;
      }
    }

    return { parentSessions: parents, childrenMap: children, hasActive: active };
  }, [project.sessions, busySessions, pinnedSessions]);

  // Filter by search
  const filteredParents = useMemo(() => {
    if (!searchQuery) return parentSessions;
    return parentSessions.filter((s) => {
      const title = (s.title || s.id).toLowerCase();
      return title.includes(searchQuery);
    });
  }, [parentSessions, searchQuery]);

  const visibleParents = showMore
    ? filteredParents
    : filteredParents.slice(0, MAX_VISIBLE_SESSIONS);
  const hasMore = filteredParents.length > MAX_VISIBLE_SESSIONS && !showMore;

  return (
    <div className="sb-project">
      {/* Project header */}
      <button
        className={`sb-project-header ${isActiveProject ? "active" : ""}`}
        onClick={onToggleExpand}
      >
        <span className="sb-project-chevron">
          {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </span>
        <span className="sb-project-name">{project.name}</span>
        {hasActive && <span className="sb-activity-dot" />}
        {project.git_branch && (
          <span className="sb-project-branch">
            <GitBranch size={10} />
            {project.git_branch}
          </span>
        )}
        <span className="sb-project-count">{parentSessions.length}</span>
      </button>

      {/* Sessions */}
      {isExpanded && (
        <div className="sb-sessions">
          {visibleParents.length === 0 ? (
            <div className="sb-empty">
              {searchQuery ? "No matching sessions" : "No sessions yet"}
            </div>
          ) : (
            visibleParents.map((session) => {
              const subagents = childrenMap.get(session.id) || [];
              const hasSubagents = subagents.length > 0;
              const isSubagentsOpen = expandedSubagents === session.id;
              const isBusy = busySessions.has(session.id);
              const hasActiveSubagent = subagents.some((s) =>
                busySessions.has(s.id)
              );
              const isActive = session.id === activeSessionId;
              const isRenaming = renameTarget?.sessionId === session.id;
              const isPinned = pinnedSessions.has(session.id);

              return (
                <div key={session.id} className="sb-session-group">
                  <button
                    className={`sb-session ${isActive ? "active" : ""} ${isBusy || hasActiveSubagent ? "busy" : ""}`}
                    onClick={() => !isRenaming && onSelectSession(session.id, index)}
                    onContextMenu={(e) =>
                      onContextMenu(
                        e,
                        session.id,
                        session.title || session.id.slice(0, 12),
                        index
                      )
                    }
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
                            // Small delay to allow submit click to register
                            setTimeout(() => {
                              if (!renameLoading) onRenameCancel();
                            }, 150);
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
                            {hasSubagents && (
                              <span
                                className="sb-subagent-badge"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  onToggleSubagents(session.id);
                                }}
                                title={`${subagents.length} subagent${subagents.length > 1 ? "s" : ""}`}
                              >
                                <Zap size={8} />
                                {subagents.length}
                              </span>
                            )}
                          </span>
                        </>
                      )}
                    </div>
                    {(isBusy || hasActiveSubagent) && (
                      <span className="sb-busy-indicator" />
                    )}
                    {/* More actions button (visible on hover) */}
                    {!isRenaming && (
                      <span
                        className="sb-session-actions"
                        onClick={(e) =>
                          onContextMenu(
                            e,
                            session.id,
                            session.title || session.id.slice(0, 12),
                            index
                          )
                        }
                      >
                        <MoreHorizontal size={14} />
                      </span>
                    )}
                  </button>

                  {/* Subagents (expanded) */}
                  {hasSubagents && isSubagentsOpen && (
                    <div className="sb-subagents">
                      {subagents.map((sub) => {
                        const subBusy = busySessions.has(sub.id);
                        const subActive = sub.id === activeSessionId;
                        return (
                          <button
                            key={sub.id}
                            className={`sb-session sb-session-sub ${subActive ? "active" : ""} ${subBusy ? "busy" : ""}`}
                            onClick={() => onSelectSession(sub.id, index)}
                          >
                            <div className="sb-session-icon sub">
                              <Zap size={12} />
                            </div>
                            <div className="sb-session-info">
                              <span className="sb-session-title">
                                {sub.title || sub.id.slice(0, 12)}
                              </span>
                              <span className="sb-session-meta">
                                {formatTime(sub.time.updated)}
                              </span>
                            </div>
                            {subBusy && <span className="sb-busy-indicator" />}
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })
          )}

          {/* Show more */}
          {hasMore && (
            <button className="sb-show-more" onClick={onShowMore}>
              Show {filteredParents.length - MAX_VISIBLE_SESSIONS} more
            </button>
          )}
        </div>
      )}
    </div>
  );
}
