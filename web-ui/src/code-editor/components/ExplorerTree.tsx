/**
 * ExplorerTree — recursive file tree used in the desktop explorer sidebar.
 * Supports inline create / rename / delete actions + download.
 */
import { useState, useRef, useEffect } from "react";
import {
  Folder, File, ChevronRight, ChevronDown, Loader2,
  FilePlus, FolderPlus, Trash2, X, RefreshCw, Pencil, Download,
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
  onReloadDir?: (dirPath: string) => void;
  onReloadFile?: (filePath: string) => void;
  onRename?: (oldPath: string, newName: string, isDir: boolean) => void;
  onDownloadFile?: (path: string) => void;
  onDownloadDir?: (path: string) => void;
}

export function ExplorerTree({
  entries, expandedDirs, dirChildren, loadingDirs,
  activeFilePath, toggleDir, onFileClick,
  onCreateFile, onCreateDir, onDeleteFile, onDeleteDir,
  onReloadDir, onReloadFile, onRename, onDownloadFile, onDownloadDir,
}: Props) {
  const [inlineCreate, setInlineCreate] = useState<{ parentDir: string; type: "file" | "dir" } | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<{ path: string; isDir: boolean } | null>(null);
  const [inlineRename, setInlineRename] = useState<{ path: string; isDir: boolean } | null>(null);
  const [contextMenu, setContextMenu] = useState<string | null>(null);

  return <>{entries.map((entry) => renderTreeNode(entry, 0))}</>;

  function renderTreeNode(entry: FileEntry, depth: number) {
    if (entry.is_dir) return <DirNode key={entry.path} entry={entry} depth={depth} />;
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
          <button type="button"
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
          {showCtx && (
            <span className="explorer-entry-actions">
              {onRename && (
                <button type="button" className="explorer-action-btn" title="Rename" onClick={(e) => { e.stopPropagation(); setInlineRename({ path: entry.path, isDir: true }); setContextMenu(null); }}>
                  <Pencil size={12} />
                </button>
              )}
              {onReloadDir && (
                <button type="button" className="explorer-action-btn" title="Reload folder" onClick={(e) => { e.stopPropagation(); onReloadDir(entry.path); setContextMenu(null); }}>
                  <RefreshCw size={12} />
                </button>
              )}
              {onDownloadDir && (
                <button type="button" className="explorer-action-btn" title="Download as zip" onClick={(e) => { e.stopPropagation(); onDownloadDir(entry.path); setContextMenu(null); }}>
                  <Download size={12} />
                </button>
              )}
              {onCreateFile && (
                <button type="button" className="explorer-action-btn" title="New file" onClick={(e) => { e.stopPropagation(); setInlineCreate({ parentDir: entry.path, type: "file" }); setContextMenu(null); }}>
                  <FilePlus size={12} />
                </button>
              )}
              {onCreateDir && (
                <button type="button" className="explorer-action-btn" title="New folder" onClick={(e) => { e.stopPropagation(); setInlineCreate({ parentDir: entry.path, type: "dir" }); setContextMenu(null); }}>
                  <FolderPlus size={12} />
                </button>
              )}
              {onDeleteDir && (
                <button type="button" className="explorer-action-btn explorer-action-danger" title="Delete folder" onClick={(e) => { e.stopPropagation(); setConfirmDelete({ path: entry.path, isDir: true }); setContextMenu(null); }}>
                  <Trash2 size={12} />
                </button>
              )}
            </span>
          )}
        </div>
        {/* Inline rename overlay */}
        {inlineRename?.path === entry.path && (
          <InlineRenameInput
            currentName={entry.name}
            isDir
            depth={depth}
            onSubmit={(newName) => { onRename?.(entry.path, newName, true); setInlineRename(null); }}
            onCancel={() => setInlineRename(null)}
          />
        )}
        {confirmDelete?.path === entry.path && (
          <ConfirmDeleteInline
            path={entry.path} isDir
            onConfirm={() => { onDeleteDir?.(entry.path); setConfirmDelete(null); }}
            onCancel={() => setConfirmDelete(null)} depth={depth}
          />
        )}
        {inlineCreate?.parentDir === entry.path && (
          <InlineCreateInput
            type={inlineCreate.type} depth={depth + 1}
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
          <button type="button"
            className={`explorer-tree-entry explorer-tree-file ${isActive ? "active" : ""}`}
            style={{ paddingLeft: `${8 + depth * 14 + 14}px` }}
            onClick={() => onFileClick(entry.path)}
          >
            <File size={14} className="file-icon" />
            <span className="file-name">{entry.name}</span>
          </button>
          {showCtx && (
            <span className="explorer-entry-actions">
              {onRename && (
                <button type="button" className="explorer-action-btn" title="Rename" onClick={(e) => { e.stopPropagation(); setInlineRename({ path: entry.path, isDir: false }); setContextMenu(null); }}>
                  <Pencil size={12} />
                </button>
              )}
              {onReloadFile && (
                <button type="button" className="explorer-action-btn" title="Reload file" onClick={(e) => { e.stopPropagation(); onReloadFile(entry.path); setContextMenu(null); }}>
                  <RefreshCw size={12} />
                </button>
              )}
              {onDownloadFile && (
                <button type="button" className="explorer-action-btn" title="Download file" onClick={(e) => { e.stopPropagation(); onDownloadFile(entry.path); setContextMenu(null); }}>
                  <Download size={12} />
                </button>
              )}
              {onDeleteFile && (
                <button type="button" className="explorer-action-btn explorer-action-danger" title="Delete file" onClick={(e) => { e.stopPropagation(); setConfirmDelete({ path: entry.path, isDir: false }); setContextMenu(null); }}>
                  <Trash2 size={12} />
                </button>
              )}
            </span>
          )}
        </div>
        {inlineRename?.path === entry.path && (
          <InlineRenameInput
            currentName={entry.name}
            isDir={false}
            depth={depth}
            onSubmit={(newName) => { onRename?.(entry.path, newName, false); setInlineRename(null); }}
            onCancel={() => setInlineRename(null)}
          />
        )}
        {confirmDelete?.path === entry.path && (
          <ConfirmDeleteInline
            path={entry.path} isDir={false}
            onConfirm={() => { onDeleteFile?.(entry.path); setConfirmDelete(null); }}
            onCancel={() => setConfirmDelete(null)} depth={depth}
          />
        )}
      </>
    );
  }
}

// ── Inline creation input ───────────────────────────────

function InlineCreateInput({ type, depth, onSubmit, onCancel }: {
  type: "file" | "dir"; depth: number;
  onSubmit: (name: string) => void; onCancel: () => void;
}) {
  const [value, setValue] = useState("");
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => { ref.current?.focus(); }, []);

  const handleSubmit = () => {
    const trimmed = value.trim();
    if (trimmed) onSubmit(trimmed); else onCancel();
  };

  return (
    <div className="explorer-inline-input" style={{ paddingLeft: `${8 + depth * 14}px` }}>
      {type === "dir" ? <FolderPlus size={13} className="file-icon folder-icon" /> : <FilePlus size={13} className="file-icon" />}
      <input
        ref={ref} className="explorer-inline-name-input" value={value}
        placeholder={type === "file" ? "filename" : "folder name"}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") handleSubmit(); if (e.key === "Escape") onCancel(); }}
        onBlur={handleSubmit}
      />
    </div>
  );
}

// ── Inline rename input ─────────────────────────────────

function InlineRenameInput({ currentName, isDir, depth, onSubmit, onCancel }: {
  currentName: string; isDir: boolean; depth: number;
  onSubmit: (newName: string) => void; onCancel: () => void;
}) {
  const [value, setValue] = useState(currentName);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus();
    // Smart selection: select filename without extension for files, full for dirs
    if (!isDir) {
      const dot = currentName.lastIndexOf(".");
      if (dot > 0) el.setSelectionRange(0, dot);
      else el.select();
    } else {
      el.select();
    }
  }, [currentName, isDir]);

  const handleSubmit = () => {
    const trimmed = value.trim();
    if (trimmed && trimmed !== currentName) onSubmit(trimmed);
    else onCancel();
  };

  return (
    <div className="explorer-inline-input" style={{ paddingLeft: `${8 + depth * 14}px` }}>
      <Pencil size={13} className="file-icon" />
      <input
        ref={ref} className="explorer-inline-name-input" value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") handleSubmit(); if (e.key === "Escape") onCancel(); }}
        onBlur={handleSubmit}
      />
    </div>
  );
}

function ConfirmDeleteInline({ path, isDir, onConfirm, onCancel, depth }: {
  path: string; isDir: boolean; onConfirm: () => void; onCancel: () => void; depth: number;
}) {
  const name = path.split("/").pop() || path;
  return (
    <div className="explorer-confirm-delete" style={{ paddingLeft: `${8 + depth * 14}px` }}>
      <span className="explorer-confirm-text">Delete {isDir ? "folder" : ""} <strong>{name}</strong>?</span>
      <button type="button" className="explorer-confirm-yes" onClick={onConfirm}>Delete</button>
      <button type="button" className="explorer-confirm-no" onClick={onCancel}><X size={12} /></button>
    </div>
  );
}
