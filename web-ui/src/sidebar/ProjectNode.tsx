import React, { useMemo } from "react";
import type { ProjectInfo, SessionInfo } from "../api";
import {
  ChevronDown,
  ChevronRight,
  Plus,
  Zap,
  GitBranch,
  SquareKanban,
} from "lucide-react";
import { SessionRow } from "./SessionRow";
import { formatTime } from "./formatTime";
import type { SessionTaskLink } from "./useSessionTaskLinks";

const MAX_VISIBLE_SESSIONS = 8;

/** A kanban task and the (root) sessions launched from it — single-session
 *  tasks contribute one session, pipeline tasks one per stage. */
interface TaskGroup {
  taskId: string;
  taskTitle: string;
  sessions: SessionInfo[];
}

export interface ProjectNodeProps {
  project: ProjectInfo;
  index: number;
  isActiveProject: boolean;
  isExpanded: boolean;
  activeSessionId: string | null;
  isSessionBusy: (sid: string) => boolean;
  /** Serialized key — changes when busy set changes, forces useMemo recomputation. */
  busyKey: string;
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
  /** session_id → originating kanban task/lane (active project only). */
  sessionTaskLinks?: Map<string, SessionTaskLink>;
  /** Open the originating kanban task's editor (clicking a task-group header). */
  onOpenKanbanTask?: (taskId: string) => void;
}

export function ProjectNode({
  project,
  index,
  isActiveProject,
  isExpanded,
  activeSessionId,
  isSessionBusy,
  busyKey,
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
  sessionTaskLinks,
  onOpenKanbanTask,
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
      if (isSessionBusy(s.id)) { active = true; break; }
    }

    return { parentSessions: parents, childrenMap: children, hasActive: active };
  }, [project.sessions, busyKey, pinnedSessions]);

  const filteredParents = useMemo(() => {
    if (!searchQuery) return parentSessions;
    return parentSessions.filter((s) =>
      (s.title || s.id).toLowerCase().includes(searchQuery),
    );
  }, [parentSessions, searchQuery]);

  // Split parents into kanban-task groups (shown grouped under the task title)
  // and the rest. Task-linked sessions are pulled out of the plain list so they
  // appear only under their task — keeping the normal list free of them.
  const { taskGroups, ungroupedParents } = useMemo(() => {
    if (!sessionTaskLinks || sessionTaskLinks.size === 0) {
      return { taskGroups: [] as TaskGroup[], ungroupedParents: filteredParents };
    }
    const byTask = new Map<string, TaskGroup>();
    const ungrouped: SessionInfo[] = [];
    for (const s of filteredParents) {
      const link = sessionTaskLinks.get(s.id);
      if (!link) { ungrouped.push(s); continue; }
      let group = byTask.get(link.taskId);
      if (!group) {
        group = {
          taskId: link.taskId,
          taskTitle: link.taskTitle || s.title || s.id.slice(0, 12),
          sessions: [],
        };
        byTask.set(link.taskId, group);
      }
      group.sessions.push(s);
    }
    // filteredParents is already sorted (pinned, then recency), so group order
    // and per-group order follow that ordering by first-seen insertion.
    return { taskGroups: Array.from(byTask.values()), ungroupedParents: ungrouped };
  }, [filteredParents, sessionTaskLinks]);

  const visibleParents = showMore
    ? ungroupedParents
    : ungroupedParents.slice(0, MAX_VISIBLE_SESSIONS);
  const hasMore = ungroupedParents.length > MAX_VISIBLE_SESSIONS && !showMore;
  const isEmpty = taskGroups.length === 0 && ungroupedParents.length === 0;

  const renderSession = (session: SessionInfo) => {
    const subagents = childrenMap.get(session.id) || [];
    const isSubagentsOpen = expandedSubagents === session.id;
    const title = session.title || session.id.slice(0, 12);

    return (
      <div key={session.id} className="sb-session-group">
        <SessionRow
          session={session}
          isActive={session.id === activeSessionId}
          isBusy={isSessionBusy(session.id)}
          hasActiveSubagent={subagents.some((s) => isSessionBusy(s.id))}
          isPinned={pinnedSessions.has(session.id)}
          isRenaming={renameTarget?.sessionId === session.id}
          subagentCount={subagents.length}
          taskLink={sessionTaskLinks?.get(session.id)}
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
                className={`sb-session sb-session-sub${sub.id === activeSessionId ? " active" : ""}${isSessionBusy(sub.id) ? " busy" : ""}`}
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
                {isSessionBusy(sub.id) && <span className="sb-busy-indicator" />}
              </button>
            ))}
          </div>
        )}
      </div>
    );
  };

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

          {/* Kanban task groups — all sessions launched from a task, grouped
              under its title. Subsessions render nested via renderSession. */}
          {taskGroups.map((group) => (
            <div key={group.taskId} className="sb-task-group">
              <button
                type="button"
                className="sb-task-group-header"
                onClick={onOpenKanbanTask ? () => onOpenKanbanTask(group.taskId) : undefined}
                title={`Kanban task · ${group.taskTitle}`}
              >
                <SquareKanban size={12} className="sb-task-group-icon" />
                <span className="sb-task-group-title">{group.taskTitle}</span>
                <span className="sb-task-group-count">{group.sessions.length}</span>
              </button>
              <div className="sb-task-group-sessions">
                {group.sessions.map(renderSession)}
              </div>
            </div>
          ))}

          {isEmpty ? (
            <div className="sb-empty">
              {searchQuery ? "No matching sessions" : "No sessions yet"}
            </div>
          ) : (
            visibleParents.map(renderSession)
          )}

          {hasMore && (
            <button className="sb-show-more" onClick={onShowMore}>
              Show {ungroupedParents.length - MAX_VISIBLE_SESSIONS} more
            </button>
          )}
        </div>
      )}
    </div>
  );
}
