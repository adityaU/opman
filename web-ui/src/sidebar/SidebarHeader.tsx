/**
 * SidebarHeader — one row that names the workspace and switches it.
 *
 * There were two stacked headers saying the same thing: an eyebrow reading
 * "Workspace" over the project name, and immediately beneath it a project row
 * repeating that name with its branch and count. Between them they spent two
 * bands and a label on one fact. This is that fact once — name, branch, session
 * count — and it is also the control that changes it, so switching a project no
 * longer needs a tree. "Add project" lives in the same menu, which is where
 * someone looking to change workspace would go for it anyway.
 */
import { useEffect, useRef, useState } from "react";
import {
  PanelLeft, LayoutGrid, Search, Plus, X, ChevronDown, GitBranch, FolderPlus, Check,
} from "lucide-react";
import type { ProjectInfo } from "../api";

interface Props {
  projects: ProjectInfo[];
  activeProject: number;
  sessionCount: number;
  searchVisible: boolean;
  isKanbanView?: boolean;
  onToggleSearch: () => void;
  onToggleKanban?: () => void;
  onToggleSidebar: () => void;
  onNewSession: () => void;
  onSwitchProject: (index: number) => void;
  onOpenAddProject: () => void;
  onClose: () => void;
}

export function SidebarHeader({
  projects, activeProject, sessionCount, searchVisible, isKanbanView,
  onToggleSearch, onToggleKanban, onToggleSidebar, onNewSession,
  onSwitchProject, onOpenAddProject, onClose,
}: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);
  const project = projects[activeProject];

  useEffect(() => {
    if (!menuOpen) return;
    const dismiss = (event: Event) => {
      if (wrapRef.current?.contains(event.target as Node)) return;
      setMenuOpen(false);
    };
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") setMenuOpen(false); };
    document.addEventListener("mousedown", dismiss);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", dismiss);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [menuOpen]);

  return (
    <div className="sb-header">
      <div className="sb-project-switch" ref={wrapRef}>
        <button
          type="button"
          className={`sb-project-trigger${menuOpen ? " is-open" : ""}`}
          onClick={() => setMenuOpen((open) => !open)}
          title={project?.path || "Choose workspace"}
          aria-haspopup="menu"
          aria-expanded={menuOpen}
        >
          {/* Name and count only. At the sidebar's real width the branch was
              stealing characters from the workspace name — the one thing this
              row exists to say. It is in the menu below, and in the git panel. */}
          <span className="sb-project-label">{project?.name || "Sessions"}</span>
          {sessionCount > 0 && <span className="sb-project-count">{sessionCount}</span>}
          <ChevronDown size={11} className="sb-project-caret" />
        </button>
        {menuOpen && (
          <div className="sb-project-menu modal-popover-surface" role="menu">
            {projects.map((entry, index) => (
              <button
                key={entry.path || index}
                type="button"
                role="menuitemradio"
                aria-checked={index === activeProject}
                className={`sb-project-item${index === activeProject ? " is-active" : ""}`}
                onClick={() => { onSwitchProject(index); setMenuOpen(false); }}
              >
                <span className="sb-project-item-name">{entry.name}</span>
                {entry.git_branch && (
                  <span className="sb-project-item-branch">
                    <GitBranch size={9} /> {entry.git_branch}
                  </span>
                )}
                {index === activeProject && <Check size={12} className="sb-project-item-check" />}
              </button>
            ))}
            <button
              type="button"
              role="menuitem"
              className="sb-project-item sb-project-add"
              onClick={() => { onOpenAddProject(); setMenuOpen(false); }}
            >
              <FolderPlus size={12} />
              <span>Add project…</span>
            </button>
          </div>
        )}
      </div>

      <div className="sb-header-actions">
        <button
          type="button"
          className={`sb-icon-btn${searchVisible ? " sb-icon-btn-active" : ""}`}
          onClick={onToggleSearch}
          title="Filter sessions"
          aria-label="Filter sessions"
          aria-pressed={searchVisible}
        >
          <Search size={14} />
        </button>
        {onToggleKanban && (
          <button
            type="button"
            className={`sb-icon-btn${isKanbanView ? " sb-icon-btn-active" : ""}`}
            onClick={onToggleKanban}
            title={isKanbanView ? "Back to chat" : "Open Kanban board"}
            aria-label="Toggle Kanban board"
            aria-pressed={isKanbanView}
          >
            <LayoutGrid size={14} />
          </button>
        )}
        <button
          type="button"
          className="sb-icon-btn sb-new-btn"
          onClick={onNewSession}
          title="New session"
          aria-label="New session"
        >
          <Plus size={14} />
        </button>
        <button
          type="button"
          className="sb-icon-btn sb-collapse-btn"
          onClick={onToggleSidebar}
          title="Hide sidebar (Cmd+B)"
          aria-label="Hide sidebar"
        >
          <PanelLeft size={14} />
        </button>
        <button type="button" className="sidebar-close-btn" onClick={onClose} aria-label="Close sidebar">
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
