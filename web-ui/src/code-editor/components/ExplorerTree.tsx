/**
 * ExplorerTree — recursive file tree used in the desktop explorer sidebar.
 * Supports inline create-file / create-folder / delete actions.
 */
import { useState, useRef, useEffect } from "react";
import {
  Folder, File, ChevronRight, ChevronDown, Loader2,
  FilePlus, FolderPlus, Trash2, MoreHorizontal, X,
} from "lucide-react";
import type { FileEntry } from "../types";

interface Props {
  entries: FileEntry[];
  expandedDirs: Set<string>;
  dirChildren: Record<string, FileEntry[]>;
  loadingDirs: Set<string>;
  activeFilePath: string | null;
  toggleDir: (dirPath: string) => void;
  onFileClick: (path: string) => void;
  // File management
  onCreateFile?: (parentDir: string, name: string) => void;
  onCreateDir?: (parentDir: string, name: string) => void;
  onDeleteFile?: (path: string) => void;
  onDeleteDir?: (path: string) => void;
}

export function ExplorerTree({
  entries, expandedDirs, dirChildren, loadingDirs,
  activeFilePath, toggleDir, onFileClick,
  onCreateFile, onCreateDir, onDeleteFile, onDeleteDir,
}: Props) {
  // Inline create state: which directory + type
  const [inlineCreate, setInlineCreate] = useState<{ parentDir: string; type: "file" | "dir" } | null>(null);
  // Confirm-delete state
  const [confirmDelete, setConfirmDelete] = useState<{ path: string; isDir: boolean } | null>(null);
  // Context menu on hover
  const [contextMenu, setContextMenu] = useState<string | null>(null);

  return <>{entries.map((entry) => renderTreeNode(entry, 0))}</>;

  function renderTreeNode(entry: FileEntry, depth: number) {
    if (entry.is_dir) {
      return <DirNode key={entry.path} entry={entry} depth={depth} />;
    }
    return <FileNode key={entry.path} entry={entry} depth={depth} />;
  }

  function DirNode({ entry, depth }: { entry: FileEntry; depth: number }) {
    const isExpanded = expandedDirs.has(entry.path);
    const isLoading = loadingDirs.has(entry.path);
    const children = dirChildren[entry.path] || [];
    const showCtx = contextMenu === entry.path;

    return (
      <div>
        <div
          className="explorer-tree-entry-row"
          onMouseEnter={() => setContextMenu(entry.path)}
          onMouseLeave={() => { if (contextMenu === entry.path) setContextMenu(null); }}
        >
          <button
            className="explorer-tree-entry explorer-tree-dir"
            style={{ paddingLeft: `${8 + depth * 14}px` }}
            onClick={() => toggleDir(entry.path)}
          >
            {isLoading ? (
              <Loader2 size={12} className="spin explorer-tree-chevron" />
            ) : isExpanded ? (
              <ChevronDown size={12} className="explorer-tree-chevron" />
            ) : (
              <ChevronRight size={12} className="explorer-tree-chevron" />
            )}
            <Folder size={14} className="file-icon folder-icon" />
            <span className="file-name">{entry.name}</span>
          </button>
          {showCtx && (onCreateFile || onCreateDir || onDeleteDir) && (
            <span className="explorer-entry-actions">
              {onCreateFile && (
                <button className="explorer-action-btn" title="New file" onClick={(e) => { e.stopPropagation(); setInlineCreate({ parentDir: entry.path, type: "file" }); setContextMenu(null); }}>
                  <FilePlus size={12} />
                </button>
              )}
              {onCreateDir && (
                <button className="explorer-action-btn" title="New folder" onClick={(e) => { e.stopPropagation(); setInlineCreate({ parentDir: entry.path, type: "dir" }); setContextMenu(null); }}>
                  <FolderPlus size={12} />
                </button>
              )}
              {onDeleteDir && (
                <button className="explorer-action-btn explorer-action-danger" title="Delete folder" onClick={(e) => { e.stopPropagation(); setConfirmDelete({ path: entry.path, isDir: true }); setContextMenu(null); }}>
                  <Trash2 size={12} />
                </button>
              )}
            </span>
          )}
        </div>
        {/* Confirm delete overlay */}
        {confirmDelete && confirmDelete.path === entry.path && (
          <ConfirmDeleteInline
            path={entry.path}
            isDir
            onConfirm={() => { onDeleteDir?.(entry.path); setConfirmDelete(null); }}
            onCancel={() => setConfirmDelete(null)}
            depth={depth}
          />
        )}
        {/* Inline create input */}
        {inlineCreate && inlineCreate.parentDir === entry.path && (
          <InlineCreateInput
            type={inlineCreate.type}
            depth={depth + 1}
            onSubmit={(name) => {
              if (inlineCreate.type === "file") onCreateFile?.(entry.path, name);
              else onCreateDir?.(entry.path, name);
              setInlineCreate(null);
            }}
            onCancel={() => setInlineCreate(null)}
          />
        )}
        {isExpanded && children.length > 0 && (
          <div className="explorer-tree-children">
            {children.map((child) => renderTreeNode(child, depth + 1))}
          </div>
        )}
      </div>
    );
  }

  function FileNode({ entry, depth }: { entry: FileEntry; depth: number }) {
    const isActive = activeFilePath === entry.path;
    const showCtx = contextMenu === entry.path;
    return (
      <>
        <div
          className="explorer-tree-entry-row"
          onMouseEnter={() => setContextMenu(entry.path)}
          onMouseLeave={() => { if (contextMenu === entry.path) setContextMenu(null); }}
        >
          <button
            className={`explorer-tree-entry explorer-tree-file ${isActive ? "active" : ""}`}
            style={{ paddingLeft: `${8 + depth * 14 + 14}px` }}
            onClick={() => onFileClick(entry.path)}
          >
            <File size={14} className="file-icon" />
            <span className="file-name">{entry.name}</span>
          </button>
          {showCtx && onDeleteFile && (
            <span className="explorer-entry-actions">
              <button className="explorer-action-btn explorer-action-danger" title="Delete file" onClick={(e) => { e.stopPropagation(); setConfirmDelete({ path: entry.path, isDir: false }); setContextMenu(null); }}>
                <Trash2 size={12} />
              </button>
            </span>
          )}
        </div>
        {confirmDelete && confirmDelete.path === entry.path && (
          <ConfirmDeleteInline
            path={entry.path}
            isDir={false}
            onConfirm={() => { onDeleteFile?.(entry.path); setConfirmDelete(null); }}
            onCancel={() => setConfirmDelete(null)}
            depth={depth}
          />
        )}
      </>
    );
  }
}

// ── Inline creation input ───────────────────────────────

function InlineCreateInput({
  type, depth, onSubmit, onCancel,
}: {
  type: "file" | "dir"; depth: number;
  onSubmit: (name: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState("");
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => { ref.current?.focus(); }, []);

  const handleSubmit = () => {
    const trimmed = value.trim();
    if (trimmed) onSubmit(trimmed);
    else onCancel();
  };

  return (
    <div className="explorer-inline-input" style={{ paddingLeft: `${8 + depth * 14}px` }}>
      {type === "dir" ? <FolderPlus size={13} className="file-icon folder-icon" /> : <FilePlus size={13} className="file-icon" />}
      <input
        ref={ref}
        className="explorer-inline-name-input"
        value={value}
        placeholder={type === "file" ? "filename" : "folder name"}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") handleSubmit();
          if (e.key === "Escape") onCancel();
        }}
        onBlur={handleSubmit}
      />
    </div>
  );
}

// ── Inline confirm delete ───────────────────────────────

function ConfirmDeleteInline({
  path, isDir, onConfirm, onCancel, depth,
}: {
  path: string; isDir: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  depth: number;
}) {
  const name = path.split("/").pop() || path;
  return (
    <div className="explorer-confirm-delete" style={{ paddingLeft: `${8 + depth * 14}px` }}>
      <span className="explorer-confirm-text">
        Delete {isDir ? "folder" : ""} <strong>{name}</strong>?
      </span>
      <button className="explorer-confirm-yes" onClick={onConfirm}>Delete</button>
      <button className="explorer-confirm-no" onClick={onCancel}><X size={12} /></button>
    </div>
  );
}
