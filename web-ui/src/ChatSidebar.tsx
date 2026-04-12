import React, { useState, useCallback } from "react";
import type { ProjectInfo } from "./api";
import {
  Plus,
  Search,
  X,
  FolderPlus,
} from "lucide-react";
import { ProjectNode } from "./sidebar/ProjectNode";
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
import { loadOpenSessions, saveOpenSessions } from "./sidebar/openSessions";

interface Props {
  projects: ProjectInfo[];
  activeProject: number;
  activeSessionId: string | null;
  busySessions: Set<string>;
  onSelectSession: (sessionId: string, projectIdx: number) => void;
  onNewSession: () => void;
  onSwitchProject: (index: number) => void;
  onOpenAddProject: () => void;
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
  onOpenAddProject,
  isMobileOpen,
  onClose,
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
  const [showMore, setShowMore] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchVisible, setSearchVisible] = useState(false);

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
    setShowMore(false);
  }, []);

  const toggleSubagents = useCallback((sessionId: string) => {
    setExpandedSubagents((prev) => (prev === sessionId ? null : sessionId));
  }, []);

  return (
    <>
    {/* Mobile overlay backdrop */}
    {isMobileOpen && (
      <div className="sidebar-mobile-overlay" onClick={onClose} aria-hidden="true" />
    )}
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
        {/* Open Sessions section */}
        <OpenSessionsSection
          projects={projects}
          openSessions={openSessions}
          activeSessionId={activeSessionId}
          busySessions={busySessions}
          pinnedSessions={pinnedSessions}
          onSelectSession={handleSelectSession}
          onRemoveOpen={removeOpenSession}
          onTogglePin={togglePin}
          onStartRename={triggerRename}
          onDeleteSession={triggerDelete}
          onContextMenu={handleContextMenu}
        />

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
            onSelectSession={handleSelectSession}
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
            onStartRename={triggerRename}
            pinnedSessions={pinnedSessions}
            onTogglePin={togglePin}
            onDeleteSession={triggerDelete}
          />
        ))}
      </div>

      {/* Add Project button */}
      <div className="sb-add-project">
        <button
          className="sb-add-project-btn"
          onClick={onOpenAddProject}
          title="Add Project"
        >
          <FolderPlus size={14} />
          <span>Add Project</span>
        </button>
      </div>

      {/* Context menu */}
      {contextMenu && (
        <SessionContextMenu
          menu={contextMenu}
          isPinned={pinnedSessions.has(contextMenu.sessionId)}
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
