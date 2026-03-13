import React, { useState, useMemo, useCallback, useEffect, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import type { DirEntry } from "./api";
import { addProject, browseDirs, getHomeDir } from "./api";
import {
  Search,
  X,
  Folder,
  FolderOpen,
  ArrowLeft,
  Home,
  Star,
  Plus,
} from "lucide-react";

interface Props {
  onClose: () => void;
}

export function AddProjectModal({ onClose }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [browsePath, setBrowsePath] = useState("");
  const [browseParent, setBrowseParent] = useState("");
  const [browseEntries, setBrowseEntries] = useState<DirEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [browseLoading, setBrowseLoading] = useState(false);
  const [selected, setSelected] = useState(0);

  const filterInputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  // Load home dir on mount
  useEffect(() => {
    (async () => {
      try {
        setBrowseLoading(true);
        const { path } = await getHomeDir();
        const res = await browseDirs(path);
        setBrowsePath(res.path);
        setBrowseParent(res.parent);
        setBrowseEntries(res.entries);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load directories");
      } finally {
        setBrowseLoading(false);
      }
    })();
  }, []);

  // Focus filter input when path changes
  useEffect(() => {
    filterInputRef.current?.focus();
  }, [browsePath]);

  // Browse into a directory
  const browseInto = useCallback(async (path: string) => {
    try {
      setBrowseLoading(true);
      setFilter("");
      setSelected(0);
      setError("");
      const res = await browseDirs(path);
      setBrowsePath(res.path);
      setBrowseParent(res.parent);
      setBrowseEntries(res.entries);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to browse directory");
    } finally {
      setBrowseLoading(false);
    }
  }, []);

  // Filtered entries based on search
  const filteredEntries = useMemo(() => {
    if (!filter.trim()) return browseEntries;
    const lower = filter.toLowerCase();
    return browseEntries.filter((e) => e.name.toLowerCase().includes(lower));
  }, [browseEntries, filter]);

  // Add the currently browsed directory as a project
  const handleAddCurrentDir = useCallback(async () => {
    if (!browsePath || loading) return;
    setLoading(true);
    setError("");
    try {
      await addProject(browsePath);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to add project");
    } finally {
      setLoading(false);
    }
  }, [browsePath, loading, onClose]);

  // Add a specific directory entry as a project
  const handleAddEntry = useCallback(async (entry: DirEntry) => {
    if (loading) return;
    if (entry.is_project) {
      onClose();
      return;
    }
    setLoading(true);
    setError("");
    try {
      await addProject(entry.path);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to add project");
    } finally {
      setLoading(false);
    }
  }, [loading, onClose]);

  // Scroll selected entry into view
  useEffect(() => {
    if (!listRef.current) return;
    const el = listRef.current.querySelector(".add-project-entry-selected");
    if (el) el.scrollIntoView({ block: "nearest" });
  }, [selected]);

  // Keyboard navigation
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelected((s) => Math.min(s + 1, filteredEntries.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelected((s) => Math.max(s - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (filteredEntries.length > 0 && selected < filteredEntries.length) {
          browseInto(filteredEntries[selected].path);
        }
      } else if (e.key === "Backspace" && filter === "" && browseParent) {
        e.preventDefault();
        browseInto(browseParent);
      }
    },
    [filteredEntries, selected, filter, browseParent, browseInto]
  );

  return (
    <div className="add-project-overlay" onClick={onClose}>
      <div
        ref={modalRef}
        className="add-project-modal"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="add-project-header">
          <div className="add-project-header-left">
            <FolderOpen size={15} />
            <h3>Add Project</h3>
          </div>
          <button className="add-project-close" onClick={onClose} title="Close (Esc)">
            <X size={15} />
          </button>
        </div>

        {/* Navigation bar */}
        <div className="add-project-nav">
          <button
            className="add-project-nav-btn"
            onClick={() => browseParent && browseInto(browseParent)}
            disabled={!browseParent || browseLoading}
            title="Go up (Backspace)"
          >
            <ArrowLeft size={14} />
          </button>
          <button
            className="add-project-nav-btn"
            onClick={() => browseInto("")}
            disabled={browseLoading}
            title="Home"
          >
            <Home size={14} />
          </button>
          <div className="add-project-path" title={browsePath}>
            {browsePath}
          </div>
          <button
            className="add-project-add-current"
            onClick={handleAddCurrentDir}
            disabled={loading || !browsePath}
          >
            {loading ? "Adding..." : "Add This Directory"}
          </button>
        </div>

        {/* Search / filter */}
        <div className="add-project-search">
          <Search size={13} className="add-project-search-icon" />
          <input
            ref={filterInputRef}
            className="add-project-filter"
            type="text"
            placeholder="Filter directories..."
            value={filter}
            onChange={(e) => {
              setFilter(e.target.value);
              setSelected(0);
            }}
            onKeyDown={handleKeyDown}
            disabled={browseLoading}
          />
        </div>

        {/* Directory listing (body) */}
        <div className="add-project-body" ref={listRef}>
          {browseLoading ? (
            <div className="add-project-empty">Loading...</div>
          ) : filteredEntries.length === 0 ? (
            <div className="add-project-empty">
              {filter ? "No matching directories" : "No subdirectories"}
            </div>
          ) : (
            filteredEntries.map((entry, i) => (
              <div
                key={entry.path}
                className={`add-project-entry${i === selected ? " add-project-entry-selected" : ""}${entry.is_project ? " add-project-entry-existing" : ""}`}
                onClick={() => browseInto(entry.path)}
                onDoubleClick={() => handleAddEntry(entry)}
                onMouseEnter={() => setSelected(i)}
                title={entry.is_project ? `${entry.path} (already added)` : entry.path}
              >
                <Folder size={14} className="add-project-entry-icon" />
                <span className="add-project-entry-name">{entry.name}</span>
                {entry.is_project && (
                  <Star size={11} className="add-project-entry-star" />
                )}
                <button
                  className="add-project-entry-add"
                  onClick={(e) => {
                    e.stopPropagation();
                    handleAddEntry(entry);
                  }}
                  title={entry.is_project ? "Already added" : "Add as project"}
                  disabled={entry.is_project || loading}
                >
                  {entry.is_project ? <Star size={11} /> : <Plus size={13} />}
                </button>
              </div>
            ))
          )}
        </div>

        {/* Error display */}
        {error && (
          <div className="add-project-error">{error}</div>
        )}

        {/* Footer */}
        <div className="add-project-footer">
          <span>Click to browse, double-click or <Plus size={11} style={{ verticalAlign: "middle" }} /> to add</span>
          <span><kbd>Backspace</kbd> Go up &nbsp; <kbd>Esc</kbd> Close</span>
        </div>
      </div>
    </div>
  );
}
