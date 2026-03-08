import React, { useState, useMemo, useCallback, useEffect, useRef } from "react";
import type { ProjectInfo, SessionInfo } from "./api";
import { deleteSession, renameSession, addProject, removeProject } from "./api";
import {
  MessageSquare,
  Plus,
  ChevronDown,
  ChevronRight,
  Search,
  GitBranch,
  X,
  Zap,
  Trash2,
  Pencil,
  MoreHorizontal,
  FolderPlus,
  Pin,
} from "lucide-react";

const MAX_VISIBLE_SESSIONS = 8;

// ── Pin persistence via localStorage ─────────────────

const PINNED_KEY = "opman-pinned-sessions";

function loadPinnedSessions(): Set<string> {
  try {
    const raw = localStorage.getItem(PINNED_KEY);
    if (raw) return new Set(JSON.parse(raw));
  } catch { /* ignore */ }
  return new Set();
}

function savePinnedSessions(pinned: Set<string>) {
  try {
    localStorage.setItem(PINNED_KEY, JSON.stringify([...pinned]));
  } catch { /* ignore */ }
}

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

export const ChatSidebar = React.memo(function ChatSidebar({
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
  const [expandedProject, setExpandedProject] = useState<number | null>(
    activeProject
  );
  const [expandedSubagents, setExpandedSubagents] = useState<string | null>(
    null
  );
  const [showMore, setShowMore] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchVisible, setSearchVisible] = useState(false);

  // Context menu state
  const [contextMenu, setContextMenu] = useState<{
    sessionId: string;
    sessionTitle: string;
    x: number;
    y: number;
    projectIdx: number;
  } | null>(null);

  // Delete confirmation state
  const [deleteConfirm, setDeleteConfirm] = useState<{
    sessionId: string;
    sessionTitle: string;
  } | null>(null);
  const [deleteLoading, setDeleteLoading] = useState(false);

  // Rename state
  const [renameTarget, setRenameTarget] = useState<{
    sessionId: string;
    currentTitle: string;
  } | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [renameLoading, setRenameLoading] = useState(false);
  const renameInputRef = useRef<HTMLInputElement>(null) as React.RefObject<HTMLInputElement>;

  // Add Project modal state
  const [addProjectOpen, setAddProjectOpen] = useState(false);
  const [addProjectPath, setAddProjectPath] = useState("");
  const [addProjectName, setAddProjectName] = useState("");
  const [addProjectLoading, setAddProjectLoading] = useState(false);
  const [addProjectError, setAddProjectError] = useState("");
  const addProjectPathRef = useRef<HTMLInputElement>(null) as React.RefObject<HTMLInputElement>;

  // Remove Project confirmation state
  const [removeConfirm, setRemoveConfirm] = useState<{
    index: number;
    name: string;
  } | null>(null);
  const [removeLoading, setRemoveLoading] = useState(false);

  // Pinned sessions
  const [pinnedSessions, setPinnedSessions] = useState<Set<string>>(loadPinnedSessions);

  const togglePin = useCallback((sessionId: string) => {
    setPinnedSessions((prev) => {
      const next = new Set(prev);
      if (next.has(sessionId)) next.delete(sessionId);
      else next.add(sessionId);
      savePinnedSessions(next);
      return next;
    });
  }, []);

  const toggleProjectExpand = useCallback((index: number) => {
    setExpandedProject((prev) => (prev === index ? null : index));
    setExpandedSubagents(null);
    setShowMore(false);
  }, []);

  const toggleSubagents = useCallback((sessionId: string) => {
    setExpandedSubagents((prev) => (prev === sessionId ? null : sessionId));
  }, []);

  // Close context menu on click outside or Escape
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

  // Focus rename input when it appears
  useEffect(() => {
    if (renameTarget && renameInputRef.current) {
      renameInputRef.current.focus();
      renameInputRef.current.select();
    }
  }, [renameTarget]);

  const handleContextMenu = useCallback(
    (
      e: React.MouseEvent,
      sessionId: string,
      sessionTitle: string,
      projectIdx: number
    ) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({ sessionId, sessionTitle, x: e.clientX, y: e.clientY, projectIdx });
    },
    []
  );

  const handleDelete = useCallback(async () => {
    if (!deleteConfirm) return;
    setDeleteLoading(true);
    try {
      await deleteSession(deleteConfirm.sessionId);
    } catch (err) {
      console.error("Failed to delete session:", err);
    } finally {
      setDeleteLoading(false);
      setDeleteConfirm(null);
    }
  }, [deleteConfirm]);

  const handleRenameSubmit = useCallback(async () => {
    if (!renameTarget || !renameValue.trim() || renameLoading) return;
    const trimmed = renameValue.trim();
    if (trimmed === renameTarget.currentTitle) {
      setRenameTarget(null);
      return;
    }
    setRenameLoading(true);
    try {
      await renameSession(renameTarget.sessionId, trimmed);
    } catch (err) {
      console.error("Failed to rename session:", err);
    } finally {
      setRenameLoading(false);
      setRenameTarget(null);
    }
  }, [renameTarget, renameValue, renameLoading]);

  const handleRenameKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleRenameSubmit();
      } else if (e.key === "Escape") {
        setRenameTarget(null);
      }
    },
    [handleRenameSubmit]
  );

  // Focus add-project path input when modal opens
  useEffect(() => {
    if (addProjectOpen && addProjectPathRef.current) {
      addProjectPathRef.current.focus();
    }
  }, [addProjectOpen]);

  const handleAddProject = useCallback(async () => {
    const path = addProjectPath.trim();
    if (!path || addProjectLoading) return;
    setAddProjectLoading(true);
    setAddProjectError("");
    try {
      await addProject(path, addProjectName.trim() || undefined);
      setAddProjectOpen(false);
      setAddProjectPath("");
      setAddProjectName("");
    } catch (err) {
      setAddProjectError(err instanceof Error ? err.message : "Failed to add project");
    } finally {
      setAddProjectLoading(false);
    }
  }, [addProjectPath, addProjectName, addProjectLoading]);

  const handleAddProjectKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAddProject();
      } else if (e.key === "Escape") {
        setAddProjectOpen(false);
      }
    },
    [handleAddProject]
  );

  const handleRemoveProject = useCallback(async () => {
    if (!removeConfirm || removeLoading) return;
    setRemoveLoading(true);
    try {
      await removeProject(removeConfirm.index);
    } catch (err) {
      console.error("Failed to remove project:", err);
    } finally {
      setRemoveLoading(false);
      setRemoveConfirm(null);
    }
  }, [removeConfirm, removeLoading]);

  return (
    <aside className={`chat-sidebar ${isMobileOpen ? "mobile-open" : ""}`}>
      {/* Header */}
      <div className="sb-header">
        <span className="sb-brand">Sessions</span>
        <div className="sb-header-actions">
          <button
            className="sb-icon-btn"
            onClick={() => setSearchVisible((v) => !v)}
            title="Search sessions"
            aria-label="Search sessions"
          >
            <Search size={14} />
          </button>
          <button
            className="sb-icon-btn sb-new-btn"
            onClick={onNewSession}
            title="New Session"
            aria-label="New session"
          >
            <Plus size={14} />
          </button>
          <button className="sidebar-close-btn" onClick={onClose} aria-label="Close sidebar">
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Search bar (collapsible) */}
      {searchVisible && (
        <div className="sb-search">
          <Search size={12} className="sb-search-icon" />
          <input
            className="sb-search-input"
            type="text"
            placeholder="Filter sessions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            autoFocus
          />
          {searchQuery && (
            <button
              className="sb-search-clear"
              onClick={() => setSearchQuery("")}
              aria-label="Clear search"
            >
              <X size={10} />
            </button>
          )}
        </div>
      )}

      {/* Session list */}
      <div className="sb-list">
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
            searchQuery={searchQuery.toLowerCase()}
            onToggleExpand={() => toggleProjectExpand(idx)}
            onSelectSession={onSelectSession}
            onNewSession={() => {
              if (idx !== activeProject) onSwitchProject(idx);
              onNewSession();
            }}
            onToggleSubagents={toggleSubagents}
            onShowMore={() => setShowMore(true)}
            onContextMenu={handleContextMenu}
            renameTarget={renameTarget}
            renameValue={renameValue}
            renameLoading={renameLoading}
            renameInputRef={renameInputRef}
            onRenameValueChange={setRenameValue}
            onRenameKeyDown={handleRenameKeyDown}
            onRenameSubmit={handleRenameSubmit}
            onRenameCancel={() => setRenameTarget(null)}
            pinnedSessions={pinnedSessions}
          />
        ))}
      </div>

      {/* Add Project button */}
      <div className="sb-add-project">
        <button
          className="sb-add-project-btn"
          onClick={() => {
            setAddProjectOpen(true);
            setAddProjectError("");
          }}
          title="Add Project"
        >
          <FolderPlus size={14} />
          <span>Add Project</span>
        </button>
      </div>

      {/* Context menu */}
      {contextMenu && (
        <div
          className="sb-context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            className="sb-context-item"
            onClick={() => {
              togglePin(contextMenu.sessionId);
              setContextMenu(null);
            }}
          >
            <Pin size={12} />
            {pinnedSessions.has(contextMenu.sessionId) ? "Unpin" : "Pin to Top"}
          </button>
          <button
            className="sb-context-item"
            onClick={() => {
              setRenameTarget({
                sessionId: contextMenu.sessionId,
                currentTitle: contextMenu.sessionTitle,
              });
              setRenameValue(contextMenu.sessionTitle);
              setContextMenu(null);
            }}
          >
            <Pencil size={12} />
            Rename
          </button>
          <button
            className="sb-context-item sb-context-danger"
            onClick={() => {
              setDeleteConfirm({
                sessionId: contextMenu.sessionId,
                sessionTitle: contextMenu.sessionTitle,
              });
              setContextMenu(null);
            }}
          >
            <Trash2 size={12} />
            Delete
          </button>
        </div>
      )}

      {/* Delete session confirmation modal */}
      {deleteConfirm && (
        <div className="sb-modal-overlay" onClick={() => !deleteLoading && setDeleteConfirm(null)}>
          <div className="sb-modal" onClick={(e) => e.stopPropagation()}>
            <div className="sb-modal-title">Delete Session</div>
            <div className="sb-modal-body">
              Are you sure you want to delete{" "}
              <strong>{deleteConfirm.sessionTitle || deleteConfirm.sessionId.slice(0, 12)}</strong>?
              This action cannot be undone.
            </div>
            <div className="sb-modal-actions">
              <button
                className="sb-modal-btn sb-modal-cancel"
                onClick={() => setDeleteConfirm(null)}
                disabled={deleteLoading}
              >
                Cancel
              </button>
              <button
                className="sb-modal-btn sb-modal-danger"
                onClick={handleDelete}
                disabled={deleteLoading}
              >
                {deleteLoading ? "Deleting..." : "Delete"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Add Project modal */}
      {addProjectOpen && (
        <div className="sb-modal-overlay" onClick={() => !addProjectLoading && setAddProjectOpen(false)}>
          <div className="sb-modal" onClick={(e) => e.stopPropagation()}>
            <div className="sb-modal-title">Add Project</div>
            <div className="sb-modal-body">
              <label className="sb-modal-label">
                Directory Path
                <input
                  ref={addProjectPathRef}
                  className="sb-modal-input"
                  type="text"
                  placeholder="/path/to/project"
                  value={addProjectPath}
                  onChange={(e) => setAddProjectPath(e.target.value)}
                  onKeyDown={handleAddProjectKeyDown}
                  disabled={addProjectLoading}
                />
              </label>
              <label className="sb-modal-label">
                Name <span className="sb-modal-optional">(optional)</span>
                <input
                  className="sb-modal-input"
                  type="text"
                  placeholder="Auto-detected from directory"
                  value={addProjectName}
                  onChange={(e) => setAddProjectName(e.target.value)}
                  onKeyDown={handleAddProjectKeyDown}
                  disabled={addProjectLoading}
                />
              </label>
              {addProjectError && (
                <div className="sb-modal-error">{addProjectError}</div>
              )}
            </div>
            <div className="sb-modal-actions">
              <button
                className="sb-modal-btn sb-modal-cancel"
                onClick={() => setAddProjectOpen(false)}
                disabled={addProjectLoading}
              >
                Cancel
              </button>
              <button
                className="sb-modal-btn sb-modal-primary"
                onClick={handleAddProject}
                disabled={addProjectLoading || !addProjectPath.trim()}
              >
                {addProjectLoading ? "Adding..." : "Add Project"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Remove Project confirmation modal */}
      {removeConfirm && (
        <div className="sb-modal-overlay" onClick={() => !removeLoading && setRemoveConfirm(null)}>
          <div className="sb-modal" onClick={(e) => e.stopPropagation()}>
            <div className="sb-modal-title">Remove Project</div>
            <div className="sb-modal-body">
              Are you sure you want to remove{" "}
              <strong>{removeConfirm.name}</strong> from your projects?
              This will not delete any files.
            </div>
            <div className="sb-modal-actions">
              <button
                className="sb-modal-btn sb-modal-cancel"
                onClick={() => setRemoveConfirm(null)}
                disabled={removeLoading}
              >
                Cancel
              </button>
              <button
                className="sb-modal-btn sb-modal-danger"
                onClick={handleRemoveProject}
                disabled={removeLoading}
              >
                {removeLoading ? "Removing..." : "Remove"}
              </button>
            </div>
          </div>
        </div>
      )}
    </aside>
  );
});

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

function ProjectNode({
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

// ── Helpers ───────────────────────────────────────────

function formatTime(epochMs: number): string {
  if (!epochMs) return "";
  const d = new Date(epochMs * 1000);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHrs = Math.floor(diffMin / 60);
  if (diffHrs < 24) return `${diffHrs}h ago`;
  const diffDays = Math.floor(diffHrs / 24);
  if (diffDays < 7) return `${diffDays}d ago`;
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
