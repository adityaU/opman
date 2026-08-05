import React, { useState, useCallback, useEffect } from "react";
import type { ProjectInfo } from "./api";
import {
  Plus,
  Search,
  X,
  FolderPlus,
  LayoutGrid,
  PanelLeft,
} from "lucide-react";
import { ProjectNode } from "./sidebar/ProjectNode";
import { SidebarHeader } from "./sidebar/SidebarHeader";
import { OpenSessionsSection } from "./sidebar/OpenSessionsSection";
import { useContextMenu, SessionContextMenu } from "./sidebar/ContextMenu";
import {
  useDeleteSession,
  useRenameSession,
  useRemoveProject,
  DeleteSessionModal,
  RemoveProjectModal,
} from "./sidebar/ConfirmModals";
import { loadPinnedSessions, savePinnedSessions } from "./sidebar/pinnedSessions";
import { loadOpenSessions, pruneOpenSessions, saveOpenSessions } from "./sidebar/openSessions";
import type { SessionTaskLink } from "./sidebar/useSessionTaskLinks";

interface Props {
  projects: ProjectInfo[];
  activeProject: number;
  activeSessionId: string | null;
  /** Stable callback — avoids passing Set reference that changes on every SSE event. */
  isSessionBusy: (sid: string) => boolean;
  /** Serialized key that changes only when the set of busy IDs actually changes.
   *  Consumed by child components that need to re-render on busy state change. */
  busyKey: string;
  onSelectSession: (sessionId: string, projectIdx: number) => void;
  onNewSession: () => void;
  onSwitchProject: (index: number) => void;
  onOpenAddProject: () => void;
  isMobileOpen: boolean;
  onClose: () => void;
  /** Whether the Kanban board view is currently active. */
  isKanbanView?: boolean;
  /** Toggle the Kanban board view for the active project. */
  onToggleKanban?: () => void;
  onToggleSidebar: () => void;
  /** session_id → originating kanban task/lane, for the active project's board. */
  sessionTaskLinks?: Map<string, SessionTaskLink>;
  /** Open the originating kanban task's editor (back-link from a session). */
  onOpenKanbanTask?: (taskId: string) => void;
}

export const ChatSidebar = React.memo(function ChatSidebar({
  projects,
  activeProject,
  activeSessionId,
  isSessionBusy,
  busyKey,
  onSelectSession,
  onNewSession,
  onSwitchProject,
  onOpenAddProject,
  isMobileOpen,
  onClose,
  isKanbanView,
  onToggleKanban,
  onToggleSidebar,
  sessionTaskLinks,
  onOpenKanbanTask,
}: Props) {
  // Auto-close sidebar on mobile after selecting a session
  const handleSelectSession = useCallback((sessionId: string, projectIdx: number) => {
    onSelectSession(sessionId, projectIdx);
    // Auto-add to open sessions (non-subagent check is in OpenSessionsSection)
    setOpenSessions((prev) => {
      if (prev.has(sessionId)) return prev;
      const next = new Set(prev);
      next.add(sessionId);
      saveOpenSessions(next);
      return next;
    });
    if (isMobileOpen) onClose();
  }, [onSelectSession, isMobileOpen, onClose]);

  // ── Local UI state ────────────────────────────────
  const [expandedProject, setExpandedProject] = useState<number | null>(activeProject);
  const [expandedSubagents, setExpandedSubagents] = useState<string | null>(null);
  // Only one kanban task-group card can be open at a time, sidebar-wide.
  const [expandedKanbanTask, setExpandedKanbanTask] = useState<string | null>(null);
  const [showMore, setShowMore] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchVisible, setSearchVisible] = useState(false);
  const toggleSearch = useCallback(() => {
    setSearchVisible((visible) => {
      if (visible) setSearchQuery("");
      return !visible;
    });
  }, []);

  // ── Pinned sessions ───────────────────────────────
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

  // ── Open sessions ─────────────────────────────────
  const [openSessions, setOpenSessions] = useState<Set<string>>(loadOpenSessions);

  useEffect(() => {
    setOpenSessions((current) => {
      const fresh = pruneOpenSessions(current, projects);
      if (fresh.size === current.size && [...fresh].every((id) => current.has(id))) return current;
      saveOpenSessions(fresh);
      return fresh;
    });
  }, [projects]);

  const removeOpenSession = useCallback((sessionId: string) => {
    setOpenSessions((prev) => {
      const next = new Set(prev);
      next.delete(sessionId);
      saveOpenSessions(next);
      return next;
    });
  }, []);

  // ── Context menu ──────────────────────────────────
  const { contextMenu, setContextMenu, handleContextMenu } = useContextMenu();

  // ── Delete / Rename / Remove hooks ────────────────
  const { deleteConfirm, setDeleteConfirm, deleteLoading, handleDelete } = useDeleteSession();
  const {
    renameTarget, setRenameTarget, renameValue, setRenameValue,
    renameLoading, renameInputRef, handleRenameSubmit, handleRenameKeyDown,
  } = useRenameSession();
  const { removeConfirm, setRemoveConfirm, removeLoading, handleRemoveProject } = useRemoveProject();

  // ── Helpers for swipe actions ─────────────────────
  const triggerRename = useCallback((sessionId: string, title: string) => {
    setRenameTarget({ sessionId, currentTitle: title });
    setRenameValue(title);
  }, [setRenameTarget, setRenameValue]);

  const triggerDelete = useCallback((sessionId: string, title: string) => {
    setDeleteConfirm({ sessionId, sessionTitle: title });
  }, [setDeleteConfirm]);

  // ── Expand / collapse ─────────────────────────────
  const toggleProjectExpand = useCallback((index: number) => {
    setExpandedProject((prev) => (prev === index ? null : index));
    setExpandedSubagents(null);
    setExpandedKanbanTask(null);
    setShowMore(false);
  }, []);

  const toggleSubagents = useCallback((sessionId: string) => {
    setExpandedSubagents((prev) => (prev === sessionId ? null : sessionId));
  }, []);

  const toggleKanbanTaskExpand = useCallback((taskId: string) => {
    setExpandedKanbanTask((prev) => (prev === taskId ? null : taskId));
  }, []);

  return (
    <>
    {/* Mobile overlay backdrop */}
    {isMobileOpen && (
      <div className="sidebar-mobile-overlay" onClick={onClose} aria-hidden="true" />
    )}
    <aside className={`chat-sidebar ${isMobileOpen ? "mobile-open" : ""}`}>
      <SidebarHeader
        projects={projects}
        activeProject={activeProject}
        sessionCount={projects[activeProject]?.sessions?.filter((s) => !s.parentID).length || 0}
        searchVisible={searchVisible}
        isKanbanView={isKanbanView}
        onToggleSearch={toggleSearch}
        onToggleKanban={onToggleKanban}
        onToggleSidebar={onToggleSidebar}
        onNewSession={onNewSession}
        onSwitchProject={onSwitchProject}
        onOpenAddProject={onOpenAddProject}
        onClose={onClose}
      />

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
        {/* Open Sessions section */}
        <OpenSessionsSection
          projects={projects}
          openSessions={openSessions}
          activeSessionId={activeSessionId}
          isSessionBusy={isSessionBusy}
          busyKey={busyKey}
          pinnedSessions={pinnedSessions}
          onSelectSession={handleSelectSession}
          onRemoveOpen={removeOpenSession}
          onTogglePin={togglePin}
          onStartRename={triggerRename}
          onDeleteSession={triggerDelete}
          onContextMenu={handleContextMenu}
          sessionTaskLinks={sessionTaskLinks}
        />

        {projects.filter((_, idx) => idx === activeProject).map((project, offset) => {
          const idx = activeProject + offset;
          return (
          <ProjectNode
            key={project.path}
            project={project}
            listedAbove={openSessions}
            chromeless
            index={idx}
            isActiveProject
            isExpanded
            activeSessionId={activeSessionId}
            isSessionBusy={isSessionBusy}
            busyKey={busyKey}
            expandedSubagents={expandedSubagents}
            showMore={showMore && expandedProject === idx}
            searchQuery={searchQuery.toLowerCase()}
            onToggleExpand={() => toggleProjectExpand(idx)}
            onSelectSession={handleSelectSession}
            onNewSession={() => {
              if (idx !== activeProject) {
                // Switch to the other project first; onSwitchProject is async so
                // we must NOT also call onNewSession here — it would fire before
                // the project switch completes and create the session in the wrong project.
                onSwitchProject(idx);
              } else {
                onNewSession();
              }
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
            onStartRename={triggerRename}
            pinnedSessions={pinnedSessions}
            onTogglePin={togglePin}
            onDeleteSession={triggerDelete}
            sessionTaskLinks={sessionTaskLinks}
            onOpenKanbanTask={onOpenKanbanTask}
            expandedKanbanTask={expandedKanbanTask}
            onToggleKanbanTaskExpand={toggleKanbanTaskExpand}
          />
          );
        })}
      </div>

      {/* Context menu */}
      {contextMenu && (
        <SessionContextMenu
          menu={contextMenu}
          isPinned={pinnedSessions.has(contextMenu.sessionId)}
          isOpen={openSessions.has(contextMenu.sessionId)}
          taskLink={sessionTaskLinks?.get(contextMenu.sessionId)}
          onOpenTask={
            onOpenKanbanTask
              ? () => {
                  const link = sessionTaskLinks?.get(contextMenu.sessionId);
                  if (link) onOpenKanbanTask(link.taskId);
                  setContextMenu(null);
                }
              : undefined
          }
          onPin={() => {
            togglePin(contextMenu.sessionId);
            setContextMenu(null);
          }}
          onRename={() => {
            triggerRename(contextMenu.sessionId, contextMenu.sessionTitle);
            setContextMenu(null);
          }}
          onDelete={() => {
            triggerDelete(contextMenu.sessionId, contextMenu.sessionTitle);
            setContextMenu(null);
          }}
          onRemoveOpen={() => {
            removeOpenSession(contextMenu.sessionId);
            setContextMenu(null);
          }}
        />
      )}

      {/* Delete session confirmation */}
      {deleteConfirm && (
        <DeleteSessionModal
          confirm={deleteConfirm}
          loading={deleteLoading}
          onClose={() => setDeleteConfirm(null)}
          onConfirm={handleDelete}
        />
      )}

      {/* Remove project confirmation */}
      {removeConfirm && (
        <RemoveProjectModal
          confirm={removeConfirm}
          loading={removeLoading}
          onClose={() => setRemoveConfirm(null)}
          onConfirm={handleRemoveProject}
        />
      )}
    </aside>
    </>
  );
});
