import React, { useState, useMemo, useCallback } from "react";
import {
  ProjectInfo,
  SessionInfo,
  switchProject,
  selectSession,
  newSession,
} from "./api";
import { SessionSearchModal } from "./SessionSearchModal";

const MAX_VISIBLE_SESSIONS = 5;

interface Props {
  projects: ProjectInfo[];
  activeProject: number;
  busySessions: Set<string>;
  onRefresh?: () => void;
  /** Mobile drawer open state */
  isDrawerOpen?: boolean;
  /** Close the mobile drawer */
  onClose?: () => void;
}

/** Separate parent sessions from subagent sessions */
function partitionSessions(sessions: SessionInfo[]) {
  const parents: SessionInfo[] = [];
  const subagentMap = new Map<string, SessionInfo[]>();

  for (const s of sessions) {
    if (!s.parentID) {
      parents.push(s);
    } else {
      const list = subagentMap.get(s.parentID) || [];
      list.push(s);
      subagentMap.set(s.parentID, list);
    }
  }

  // Sort parents by updated time (most recent first)
  parents.sort((a, b) => (b.time.updated || 0) - (a.time.updated || 0));

  // Sort subagents within each parent
  for (const [, subs] of subagentMap) {
    subs.sort((a, b) => (b.time.updated || 0) - (a.time.updated || 0));
  }

  return { parents, subagentMap };
}

/** Check if a session or any of its subagents is busy */
function isSessionBusy(
  sessionId: string,
  busySessions: Set<string>,
  subagentMap: Map<string, SessionInfo[]>
): boolean {
  if (busySessions.has(sessionId)) return true;
  const subs = subagentMap.get(sessionId);
  if (subs) {
    return subs.some((s) => busySessions.has(s.id));
  }
  return false;
}

export function Sidebar({ projects, activeProject, busySessions, onRefresh, isDrawerOpen, onClose }: Props) {
  const [expandedProject, setExpandedProject] = useState<number | null>(activeProject);
  const [expandedSubagents, setExpandedSubagents] = useState<Set<string>>(new Set());
  const [searchModalProject, setSearchModalProject] = useState<number | null>(null);

  const handleProjectClick = useCallback(
    (idx: number) => {
      if (idx === expandedProject) {
        setExpandedProject(null);
      } else {
        setExpandedProject(idx);
      }
      if (idx !== activeProject) {
        switchProject(idx).catch(() => {});
      }
    },
    [expandedProject, activeProject]
  );

  const handleSessionClick = useCallback((projIdx: number, sessionId: string) => {
    selectSession(projIdx, sessionId).catch(() => {});
  }, []);

  const handleNewSession = useCallback(
    (projIdx: number) => {
      newSession(projIdx)
        .then(() => onRefresh?.())
        .catch(() => {});
    },
    [onRefresh]
  );

  const toggleSubagents = useCallback((sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setExpandedSubagents((prev) => {
      const next = new Set(prev);
      if (next.has(sessionId)) {
        next.delete(sessionId);
      } else {
        next.add(sessionId);
      }
      return next;
    });
  }, []);

  const handleMoreClick = useCallback((projIdx: number) => {
    setSearchModalProject(projIdx);
  }, []);

  const handleSearchSelect = useCallback(
    (projIdx: number, sessionId: string) => {
      setSearchModalProject(null);
      selectSession(projIdx, sessionId).catch(() => {});
    },
    []
  );

  return (
    <div className={`sidebar ${isDrawerOpen ? "mobile-open" : ""}`}>
      <div className="sidebar-header">
        <span>Projects</span>
        {onClose && (
          <button className="sidebar-close-btn" onClick={onClose} aria-label="Close sidebar">
            \u2715
          </button>
        )}
      </div>
      <div className="sidebar-items">
        {projects.length === 0 && (
          <div className="sidebar-empty">
            <span>No projects.</span>
            <span className="sidebar-empty-hint">Add a project to get started.</span>
          </div>
        )}
        {projects.map((project, idx) => (
          <ProjectEntry
            key={project.path}
            project={project}
            idx={idx}
            isActive={idx === activeProject}
            isExpanded={expandedProject === idx}
            busySessions={busySessions}
            expandedSubagents={expandedSubagents}
            onProjectClick={handleProjectClick}
            onSessionClick={handleSessionClick}
            onNewSession={handleNewSession}
            onToggleSubagents={toggleSubagents}
            onMoreClick={handleMoreClick}
          />
        ))}
      </div>

      {/* Add Project button pinned to bottom */}
      <div className="sidebar-add-project" onClick={() => {/* TODO: add project modal */}}>
        <span className="sidebar-add-icon">+</span>
        <span>Add Project</span>
      </div>

      {/* Session search modal */}
      {searchModalProject !== null && (
        <SessionSearchModal
          project={projects[searchModalProject]}
          projectIdx={searchModalProject}
          onSelect={handleSearchSelect}
          onClose={() => setSearchModalProject(null)}
        />
      )}
    </div>
  );
}

// ── Project Entry ────────────────────────────────────────

interface ProjectEntryProps {
  project: ProjectInfo;
  idx: number;
  isActive: boolean;
  isExpanded: boolean;
  busySessions: Set<string>;
  expandedSubagents: Set<string>;
  onProjectClick: (idx: number) => void;
  onSessionClick: (projIdx: number, sessionId: string) => void;
  onNewSession: (projIdx: number) => void;
  onToggleSubagents: (sessionId: string, e: React.MouseEvent) => void;
  onMoreClick: (projIdx: number) => void;
}

function ProjectEntry({
  project,
  idx,
  isActive,
  isExpanded,
  busySessions,
  expandedSubagents,
  onProjectClick,
  onSessionClick,
  onNewSession,
  onToggleSubagents,
  onMoreClick,
}: ProjectEntryProps) {
  const { parents, subagentMap } = useMemo(
    () => partitionSessions(project.sessions),
    [project.sessions]
  );

  const visibleSessions = parents.slice(0, MAX_VISIBLE_SESSIONS);
  const hasMore = parents.length > MAX_VISIBLE_SESSIONS;

  // Check if any session in this project is busy
  const projectHasBusy = useMemo(() => {
    return project.sessions.some((s) => busySessions.has(s.id));
  }, [project.sessions, busySessions]);

  return (
    <React.Fragment>
      {/* Project row */}
      <div
        className={`sidebar-project ${isActive ? "active" : ""} ${isExpanded ? "expanded" : ""}`}
        onClick={() => onProjectClick(idx)}
      >
        <span className="sidebar-project-marker">
          {isActive ? "▶" : " "}
        </span>
        {projectHasBusy && <span className="dot busy" />}
        {!projectHasBusy && <span className="dot idle" />}
        <span className="sidebar-project-name">{project.name}</span>
        <span className="sidebar-expand-arrow">
          {isExpanded ? "▼" : "▶"}
        </span>
      </div>

      {/* Expanded content */}
      {isExpanded && (
        <div className="sidebar-project-children">
          {/* New Session button */}
          <div
            className="sidebar-new-session"
            onClick={() => onNewSession(idx)}
          >
            <span className="sidebar-new-icon">+</span>
            <span>New Session</span>
          </div>

          {/* Session list */}
          {visibleSessions.length === 0 && (
            <div className="sidebar-no-sessions">
              <span>└</span>
              <span>no sessions yet</span>
            </div>
          )}

          {visibleSessions.map((session) => {
            const subagents = subagentMap.get(session.id) || [];
            const hasSubagents = subagents.length > 0;
            const isSubagentsOpen = expandedSubagents.has(session.id);
            const sessionBusy = isSessionBusy(session.id, busySessions, subagentMap);
            const isSelected = session.id === project.active_session;

            return (
              <React.Fragment key={session.id}>
                {/* Parent session row */}
                <div
                  className={`sidebar-session ${isSelected ? "active" : ""}`}
                  onClick={() => onSessionClick(idx, session.id)}
                >
                  <span className="sidebar-session-branch">└</span>
                  {sessionBusy && <span className="dot busy" />}
                  {hasSubagents && (
                    <span
                      className={`sidebar-subagent-arrow ${isSubagentsOpen ? "expanded" : ""}`}
                      onClick={(e) => onToggleSubagents(session.id, e)}
                      title={isSubagentsOpen ? "Collapse subagents" : "Expand subagents"}
                    >
                      {isSubagentsOpen ? "▼" : "▶"}
                    </span>
                  )}
                  <span className="sidebar-session-title">
                    {session.title || session.id.slice(0, 8)}
                  </span>
                </div>

                {/* Subagent sessions */}
                {isSubagentsOpen &&
                  subagents.map((sub) => {
                    const subBusy = busySessions.has(sub.id);
                    const subSelected = sub.id === project.active_session;

                    return (
                      <div
                        key={sub.id}
                        className={`sidebar-subagent ${subSelected ? "active" : ""}`}
                        onClick={() => onSessionClick(idx, sub.id)}
                      >
                        <span className="sidebar-subagent-branch">└</span>
                        {subBusy && <span className="dot busy" />}
                        <span className="sidebar-session-title">
                          {sub.title || sub.id.slice(0, 8)}
                        </span>
                      </div>
                    );
                  })}
              </React.Fragment>
            );
          })}

          {/* "more..." link */}
          {hasMore && (
            <div
              className="sidebar-more"
              onClick={() => onMoreClick(idx)}
            >
              <span className="sidebar-session-branch">└</span>
              <span>more... ({parents.length} total)</span>
            </div>
          )}
        </div>
      )}
    </React.Fragment>
  );
}
