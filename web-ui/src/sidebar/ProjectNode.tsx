import React, { useMemo } from "react";
import type { ProjectInfo, SessionInfo } from "../api";
import {
  ChevronDown,
  ChevronRight,
  Plus,
  Zap,
  GitBranch,
} from "lucide-react";
import { SessionRow } from "./SessionRow";
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
    projectIdx: number,
  ) => void;
  renameTarget: { sessionId: string; currentTitle: string } | null;
  renameValue: string;
  renameLoading: boolean;
  renameInputRef: React.RefObject<HTMLInputElement>;
  onRenameValueChange: (value: string) => void;
  onRenameKeyDown: (e: React.KeyboardEvent) => void;
  onRenameSubmit: () => void;
  onRenameCancel: () => void;
  onStartRename: (sessionId: string, title: string) => void;
  pinnedSessions: Set<string>;
  onTogglePin: (sessionId: string) => void;
  onDeleteSession: (sessionId: string, sessionTitle: string) => void;
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
  onStartRename,
  pinnedSessions,
  onTogglePin,
  onDeleteSession,
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

    parents.sort((a, b) => {
      const ap = pinnedSessions.has(a.id) ? 1 : 0;
      const bp = pinnedSessions.has(b.id) ? 1 : 0;
      if (ap !== bp) return bp - ap;
      return b.time.updated - a.time.updated;
    });

    let active = false;
    for (const s of project.sessions) {
      if (busySessions.has(s.id)) { active = true; break; }
    }

    return { parentSessions: parents, childrenMap: children, hasActive: active };
  }, [project.sessions, busySessions, pinnedSessions]);

  const filteredParents = useMemo(() => {
    if (!searchQuery) return parentSessions;
    return parentSessions.filter((s) =>
      (s.title || s.id).toLowerCase().includes(searchQuery),
    );
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
          {/* + New Session row */}
          <button className="sb-session sb-new-session-row" onClick={onNewSession}>
            <div className="sb-session-icon"><Plus size={14} /></div>
            <div className="sb-session-info">
              <span className="sb-session-title">New Session</span>
            </div>
          </button>

          {visibleParents.length === 0 ? (
            <div className="sb-empty">
              {searchQuery ? "No matching sessions" : "No sessions yet"}
            </div>
          ) : (
            visibleParents.map((session) => {
              const subagents = childrenMap.get(session.id) || [];
              const isSubagentsOpen = expandedSubagents === session.id;
              const title = session.title || session.id.slice(0, 12);

              return (
                <div key={session.id} className="sb-session-group">
                  <SessionRow
                    session={session}
                    isActive={session.id === activeSessionId}
                    isBusy={busySessions.has(session.id)}
                    hasActiveSubagent={subagents.some((s) => busySessions.has(s.id))}
                    isPinned={pinnedSessions.has(session.id)}
                    isRenaming={renameTarget?.sessionId === session.id}
                    subagentCount={subagents.length}
                    renameValue={renameValue}
                    renameLoading={renameLoading}
                    renameInputRef={renameInputRef}
                    onSelect={() => onSelectSession(session.id, index)}
                    onContextMenu={(e) => onContextMenu(e, session.id, title, index)}
                    onToggleSubagents={() => onToggleSubagents(session.id)}
                    onRenameValueChange={onRenameValueChange}
                    onRenameKeyDown={onRenameKeyDown}
                    onRenameSubmit={onRenameSubmit}
                    onRenameCancel={onRenameCancel}
                    onSwipePin={() => onTogglePin(session.id)}
                    onSwipeRename={() => onStartRename(session.id, title)}
                    onSwipeDelete={() => onDeleteSession(session.id, title)}
                  />

                  {/* Subagents (expanded) */}
                  {subagents.length > 0 && isSubagentsOpen && (
                    <div className="sb-subagents">
                      {subagents.map((sub) => (
                        <button
                          key={sub.id}
                          className={`sb-session sb-session-sub${sub.id === activeSessionId ? " active" : ""}${busySessions.has(sub.id) ? " busy" : ""}`}
                          onClick={() => onSelectSession(sub.id, index)}
                        >
                          <div className="sb-session-icon sub"><Zap size={12} /></div>
                          <div className="sb-session-info">
                            <span className="sb-session-title">
                              {sub.title || sub.id.slice(0, 12)}
                            </span>
                            <span className="sb-session-meta">
                              {formatTime(sub.time.updated)}
                            </span>
                          </div>
                          {busySessions.has(sub.id) && <span className="sb-busy-indicator" />}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              );
            })
          )}

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
