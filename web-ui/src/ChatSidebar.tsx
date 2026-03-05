import React, { useState, useMemo, useCallback } from "react";
import type { ProjectInfo, SessionInfo } from "./api";
import {
  MessageSquare,
  Plus,
  ChevronDown,
  ChevronRight,
  FolderOpen,
  GitBranch,
  FolderPlus,
} from "lucide-react";

const MAX_VISIBLE_SESSIONS = 5;

interface Props {
  projects: ProjectInfo[];
  activeProject: number;
  activeSessionId: string | null;
  busySessions: Set<string>;
  onSelectSession: (sessionId: string, projectIdx: number) => void;
  onNewSession: () => void;
  onSwitchProject: (index: number) => void;
  isMobileOpen: boolean;
  onClose: () => void;
}

export function ChatSidebar({
  projects,
  activeProject,
  activeSessionId,
  busySessions,
  onSelectSession,
  onNewSession,
  onSwitchProject,
  isMobileOpen,
  onClose,
}: Props) {
  // Which project index has its sessions expanded (only one at a time, like TUI)
  const [expandedProject, setExpandedProject] = useState<number | null>(
    activeProject
  );
  // Which parent session has its subagents expanded (only one at a time)
  const [expandedSubagents, setExpandedSubagents] = useState<string | null>(
    null
  );
  // Show all sessions (beyond MAX_VISIBLE_SESSIONS)
  const [showMore, setShowMore] = useState(false);

  const toggleProjectExpand = useCallback(
    (index: number) => {
      setExpandedProject((prev) => (prev === index ? null : index));
      setExpandedSubagents(null);
      setShowMore(false);
    },
    []
  );

  const toggleSubagents = useCallback((sessionId: string) => {
    setExpandedSubagents((prev) => (prev === sessionId ? null : sessionId));
  }, []);

  return (
    <aside className={`chat-sidebar ${isMobileOpen ? "mobile-open" : ""}`}>
      <div className="chat-sidebar-header">
        <span className="chat-sidebar-title">Sessions</span>
        <button
          className="chat-sidebar-new-btn"
          onClick={onNewSession}
          title="New Session (Cmd+Shift+N)"
        >
          <Plus size={14} />
        </button>
        <button className="sidebar-close-btn" onClick={onClose}>
          &times;
        </button>
      </div>

      <div className="chat-sidebar-tree">
        {projects.map((project, idx) => (
          <ProjectNode
            key={project.path}
            project={project}
            index={idx}
            isActiveProject={idx === activeProject}
            isExpanded={expandedProject === idx}
            activeSessionId={activeSessionId}
            busySessions={busySessions}
            expandedSubagents={expandedSubagents}
            showMore={showMore && expandedProject === idx}
            onToggleExpand={() => toggleProjectExpand(idx)}
            onSelectSession={onSelectSession}
            onNewSession={() => {
              if (idx !== activeProject) onSwitchProject(idx);
              onNewSession();
            }}
            onToggleSubagents={toggleSubagents}
            onShowMore={() => setShowMore(true)}
          />
        ))}

        {/* Add Project placeholder (like TUI) */}
        {projects.length > 1 && (
          <div className="sidebar-add-project">
            <FolderPlus size={11} />
            <span>Add Project</span>
          </div>
        )}
      </div>
    </aside>
  );
}

// ── Project Node ──────────────────────────────────────

interface ProjectNodeProps {
  project: ProjectInfo;
  index: number;
  isActiveProject: boolean;
  isExpanded: boolean;
  activeSessionId: string | null;
  busySessions: Set<string>;
  expandedSubagents: string | null;
  showMore: boolean;
  onToggleExpand: () => void;
  onSelectSession: (sessionId: string, projectIdx: number) => void;
  onNewSession: () => void;
  onToggleSubagents: (sessionId: string) => void;
  onShowMore: () => void;
}

function ProjectNode({
  project,
  index,
  isActiveProject,
  isExpanded,
  activeSessionId,
  busySessions,
  expandedSubagents,
  showMore,
  onToggleExpand,
  onSelectSession,
  onNewSession,
  onToggleSubagents,
  onShowMore,
}: ProjectNodeProps) {
  // Build session hierarchy
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

    // Sort parents: newest first
    parents.sort((a, b) => b.time.updated - a.time.updated);

    // Check if any session in project is busy/active
    let active = false;
    for (const s of project.sessions) {
      if (busySessions.has(s.id)) {
        active = true;
        break;
      }
    }

    return { parentSessions: parents, childrenMap: children, hasActive: active };
  }, [project.sessions, busySessions]);

  const visibleParents = showMore
    ? parentSessions
    : parentSessions.slice(0, MAX_VISIBLE_SESSIONS);
  const hasMore = parentSessions.length > MAX_VISIBLE_SESSIONS && !showMore;

  return (
    <div className="sidebar-project-node">
      {/* Project row */}
      <button
        className={`sidebar-project-row ${isActiveProject ? "active" : ""}`}
        onClick={onToggleExpand}
      >
        <span className="sidebar-project-marker">
          {isActiveProject ? "\u25B6" : "\u00A0"}
        </span>
        {hasActive && <span className="dot busy sidebar-dot" />}
        <FolderOpen size={12} />
        <span className="sidebar-project-name">{project.name}</span>
        {project.git_branch && (
          <span className="sidebar-project-branch">
            <GitBranch size={9} />
            {project.git_branch}
          </span>
        )}
        <span className="sidebar-expand-arrow">
          {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </span>
      </button>

      {/* Expanded content */}
      {isExpanded && (
        <div className="sidebar-project-children">
          {/* + New Session */}
          <button
            className="sidebar-new-session-row"
            onClick={onNewSession}
          >
            <span className="sidebar-connector">{"\u2514"}</span>
            <Plus size={10} />
            <span>New Session</span>
          </button>

          {/* Sessions */}
          {visibleParents.length === 0 ? (
            <div className="sidebar-no-sessions">
              <span className="sidebar-connector">{"\u2514"}</span>
              <span>no sessions yet</span>
            </div>
          ) : (
            visibleParents.map((session, idx) => {
              const subagents = childrenMap.get(session.id) || [];
              const hasSubagents = subagents.length > 0;
              const isSubagentsOpen = expandedSubagents === session.id;
              const isBusy = busySessions.has(session.id);
              const hasActiveSubagent = subagents.some((s) =>
                busySessions.has(s.id)
              );
              const isActive = session.id === activeSessionId;
              const isLast = idx === visibleParents.length - 1 && !hasMore;

              return (
                <div key={session.id} className="sidebar-session-group">
                  {/* Parent session row */}
                  <div className="sidebar-session-row-container">
                    <button
                      className={`sidebar-session-row ${isActive ? "active" : ""}`}
                      onClick={() => onSelectSession(session.id, index)}
                    >
                      <span className="sidebar-connector">
                        {isLast ? "\u2514" : "\u251C"}
                      </span>
                      {(isBusy || hasActiveSubagent) && (
                        <span className="dot busy sidebar-dot" />
                      )}
                      {hasSubagents && (
                        <span
                          className="sidebar-subagent-toggle"
                          onClick={(e) => {
                            e.stopPropagation();
                            onToggleSubagents(session.id);
                          }}
                          title={isSubagentsOpen ? "Collapse subagents" : `Expand subagents (${subagents.length})`}
                        >
                          {isSubagentsOpen ? "\u25BC" : "\u25B6"}
                        </span>
                      )}
                      <span className="sidebar-session-title">
                        {session.title || session.id.slice(0, 12)}
                      </span>
                      <span className="sidebar-session-time">
                        {formatTime(session.time.updated)}
                      </span>
                    </button>
                  </div>

                  {/* Subagent sessions (nested) */}
                  {hasSubagents && isSubagentsOpen && (
                    <div className="sidebar-subagent-list">
                      {subagents.map((sub, subIdx) => {
                        const subBusy = busySessions.has(sub.id);
                        const subActive = sub.id === activeSessionId;
                        const subIsLast = subIdx === subagents.length - 1;
                        return (
                          <button
                            key={sub.id}
                            className={`sidebar-subagent-row ${subActive ? "active" : ""}`}
                            onClick={() => onSelectSession(sub.id, index)}
                          >
                            {/* Extra indent for subagent level */}
                            <span className="sidebar-subagent-connector">
                              {subIsLast ? "\u2514" : "\u251C"}
                            </span>
                            {subBusy && (
                              <span className="dot busy sidebar-dot" />
                            )}
                            <span className="sidebar-session-title">
                              {sub.title || sub.id.slice(0, 12)}
                            </span>
                            <span className="sidebar-session-time">
                              {formatTime(sub.time.updated)}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })
          )}

          {/* "more..." link */}
          {hasMore && (
            <button className="sidebar-more-row" onClick={onShowMore}>
              <span className="sidebar-connector">{"\u2514"}</span>
              <span>
                more... ({parentSessions.length - MAX_VISIBLE_SESSIONS} hidden)
              </span>
            </button>
          )}
        </div>
      )}
    </div>
  );
}

// ── Helpers ───────────────────────────────────────────

function formatTime(epochMs: number): string {
  if (!epochMs) return "";
  const d = new Date(epochMs * 1000);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "now";
  if (diffMin < 60) return `${diffMin}m`;
  const diffHrs = Math.floor(diffMin / 60);
  if (diffHrs < 24) return `${diffHrs}h`;
  const diffDays = Math.floor(diffHrs / 24);
  if (diffDays < 7) return `${diffDays}d`;
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
