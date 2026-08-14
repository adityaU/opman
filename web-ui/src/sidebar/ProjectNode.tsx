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
import { TaskGroupNode, type TaskGroup } from "./TaskGroupNode";
import { formatTime } from "./formatTime";
import type { SessionTaskLink } from "./useSessionTaskLinks";
import { groupSessions } from "./sessionGroups";

export interface ProjectNodeProps {
  project: ProjectInfo;
  index: number;
  isActiveProject: boolean;
  isExpanded: boolean;
  activeSessionId: string | null;
  isSessionBusy: (sid: string) => boolean;
  /** Serialized key — changes when busy set changes, forces useMemo recomputation. */
  busyKey: string;
  /**
   * Sessions already listed above, in Open Sessions. Shown once, not twice: the
   * duplicate rows were the single largest source of noise in the sidebar.
   */
  listedAbove?: Set<string>;
  /**
   * Render the sessions without the project's own header row. The sidebar header
   * already names the project and switches it, so a second row naming it is the
   * duplication this redesign removes.
   */
  chromeless?: boolean;
  expandedSubagents: string | null;
  /** Top-level sessions the server holds that have not been fetched yet. */
  remainingSessions: number;
  /** A "show more" fetch is in flight. */
  loadingMore: boolean;
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
  /** Which kanban task group is currently expanded, sidebar-wide (only one at a time). */
  expandedKanbanTask: string | null;
  /** Toggle a kanban task group's expanded state (collapses any other open one). */
  onToggleKanbanTaskExpand: (taskId: string) => void;
}

export function ProjectNode({
  project,
  listedAbove,
  chromeless,
  index,
  isActiveProject,
  isExpanded,
  activeSessionId,
  isSessionBusy,
  busyKey,
  expandedSubagents,
  remainingSessions,
  loadingMore,
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
  expandedKanbanTask,
  onToggleKanbanTaskExpand,
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

    // Recency only: pinned rows are lifted out by `groupSessions`, which holds them
    // above the date headings rather than sorting them to the top of one.
    parents.sort((a, b) => b.time.updated - a.time.updated);

    let active = false;
    for (const s of project.sessions) {
      if (isSessionBusy(s.id)) { active = true; break; }
    }

    return { parentSessions: parents, childrenMap: children, hasActive: active };
  }, [project.sessions, busyKey, pinnedSessions]);

  const filteredParents = useMemo(() => {
    const visible = listedAbove && listedAbove.size > 0
      ? parentSessions.filter((s) => !listedAbove.has(s.id))
      : parentSessions;
    if (!searchQuery) return visible;
    return visible.filter((s) =>
      (s.title || s.id).toLowerCase().includes(searchQuery),
    );
  }, [parentSessions, searchQuery, listedAbove]);

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
      // Archived tasks (and their sessions) are hidden from the sidebar entirely.
      if (link.archived) continue;
      let group = byTask.get(link.taskId);
      if (!group) {
        group = {
          taskId: link.taskId,
          taskTitle: link.taskTitle || s.title || s.id.slice(0, 12),
          // filteredParents is recency-sorted, so the first session seen carries
          // the group's most-recent lane colour (the active stage in pipelines).
          laneColor: link.laneColor,
          lastUpdated: 0,
          sessions: [],
        };
        byTask.set(link.taskId, group);
      }
      group.sessions.push(s);
      if (s.time.updated > group.lastUpdated) group.lastUpdated = s.time.updated;
    }
    // filteredParents is already sorted (pinned, then recency), so group order
    // and per-group order follow that ordering by first-seen insertion.
    return { taskGroups: Array.from(byTask.values()), ungroupedParents: ungrouped };
  }, [filteredParents, sessionTaskLinks]);

  // Bucketed against the render's own clock rather than a ticking one: the
  // headings only need to be right when the list changes, and re-grouping every
  // minute would rebuild every row for a boundary that moves once a day.
  const { pinned, groups } = useMemo(
    () => groupSessions(ungroupedParents, Date.now(), pinnedSessions),
    [ungroupedParents, pinnedSessions],
  );
  const isEmpty = taskGroups.length === 0 && ungroupedParents.length === 0;
  // Searching filters what is loaded; offering "show more" mid-search would look
  // like the search itself was paginated.
  const hasMore = remainingSessions > 0 && !searchQuery;

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
                data-list-item=""
                data-list-key={sub.id}
                data-list-depth={2}
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
      {/* Project header — omitted when the sidebar header already names it. */}
      {!chromeless && (
      <button
        className={`sb-project-header ${isActiveProject ? "active" : ""}`}
        onClick={onToggleExpand}
        aria-expanded={isExpanded}
        data-list-item=""
        data-list-key={project.path}
        data-list-depth={0}
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
        <span className="sb-project-count">
          {project.session_count ?? parentSessions.length}
        </span>
      </button>
      )}

      {/* Sessions */}
      {isExpanded && (
        <div className="sb-sessions">
          {/* No "New Session" row: it looked like a session, sat two rows under
              the header's + button, and pushed the real sessions down. Starting
              one is a header action. */}
          {/* Kanban task groups — all sessions launched from a task, grouped
              under its title. Subsessions render nested via renderSession. */}
          {taskGroups.map((group) => (
            <TaskGroupNode
              key={group.taskId}
              group={group}
              isExpanded={expandedKanbanTask === group.taskId}
              onToggleExpand={onToggleKanbanTaskExpand}
              onOpenKanbanTask={onOpenKanbanTask}
              renderSession={renderSession}
            />
          ))}

          {isEmpty && (
            <div className="sb-empty">
              {searchQuery ? "No matching sessions" : "No sessions yet"}
            </div>
          )}

          {pinned.map(renderSession)}

          {groups.map((group) => (
            <React.Fragment key={group.bucket}>
              <div className="sb-session-bucket" role="presentation">
                {group.label}
              </div>
              {group.sessions.map(renderSession)}
            </React.Fragment>
          ))}

          {hasMore && (
            <button
              className="sb-show-more"
              onClick={onShowMore}
              disabled={loadingMore}
            >
              {loadingMore ? "Loading…" : `Show ${remainingSessions} more`}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
