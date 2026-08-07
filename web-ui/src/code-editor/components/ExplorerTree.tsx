/**
 * ExplorerTree — the recursive file tree.
 *
 * Two decisions shape it. Rows carry no controls: actions open on right-click
 * or the row's single trailing button, so the panel spends its width on the
 * names it exists to show. And every row draws the indent guides for its
 * ancestors, with the guides along the active file's chain lit — the "spine" —
 * so the answer to "where am I" is visible without expanding anything.
 */
import { useCallback, useMemo, useState } from "react";
import { Folder, FolderOpen, MoreVertical, FilePlus, FolderPlus, Pencil } from "lucide-react";
import type { FileEntry } from "../types";
import { ExplorerContextMenu } from "../explorer/ExplorerContextMenu";
import { buildRowActions, type RowActionHandlers } from "../explorer/rowActions";
import {
  INDENT, FileTile, DirTile, InlineNameField, ConfirmDelete,
} from "../explorer/ExplorerBits";

interface Props extends RowActionHandlers {
  entries: FileEntry[];
  expandedDirs: Set<string>;
  dirChildren: Record<string, FileEntry[]>;
  loadingDirs: Set<string>;
  activeFilePath: string | null;
  toggleDir: (dirPath: string) => void;
  onFileClick: (path: string) => void;
}

interface Pending { path: string; isDir: boolean }
interface Creating { parentDir: string; type: "file" | "dir" }
interface CtxTarget { entry: FileEntry; x: number; y: number }

export function ExplorerTree(p: Props) {
  const [creating, setCreating] = useState<Creating | null>(null);
  const [confirming, setConfirming] = useState<Pending | null>(null);
  const [renaming, setRenaming] = useState<Pending | null>(null);
  const [ctx, setCtx] = useState<CtxTarget | null>(null);

  /** Ancestor paths of the open file — the segments the spine lights. */
  const litPaths = useMemo(() => {
    const lit = new Set<string>();
    const path = p.activeFilePath;
    if (!path) return lit;
    const parts = path.split("/");
    for (let i = parts.length - 1; i > 0; i -= 1) lit.add(parts.slice(0, i).join("/"));
    return lit;
  }, [p.activeFilePath]);

  const openCtx = useCallback((entry: FileEntry, x: number, y: number) => {
    setCtx({ entry, x, y });
  }, []);

  /** Tree keys a keyboard user expects: expand, collapse, rename, delete, menu. */
  const onRowKey = (event: React.KeyboardEvent, entry: FileEntry) => {
    const { key } = event;
    if (key === "F2" && p.onRename) {
      event.preventDefault();
      setRenaming({ path: entry.path, isDir: entry.is_dir });
      return;
    }
    if (key === "Delete" || key === "Backspace") {
      const canDelete = entry.is_dir ? p.onDeleteDir : p.onDeleteFile;
      if (!canDelete) return;
      event.preventDefault();
      setConfirming({ path: entry.path, isDir: entry.is_dir });
      return;
    }
    if (key === "ContextMenu" || (key === "." && (event.metaKey || event.ctrlKey))) {
      event.preventDefault();
      const r = (event.currentTarget as HTMLElement).getBoundingClientRect();
      openCtx(entry, r.left + 24, r.bottom + 2);
      return;
    }
    if (!entry.is_dir) return;
    const open = p.expandedDirs.has(entry.path);
    if (key === "ArrowRight" && !open) { event.preventDefault(); p.toggleDir(entry.path); }
    if (key === "ArrowLeft" && open) { event.preventDefault(); p.toggleDir(entry.path); }
  };

  const actionsFor = (entry: FileEntry) => buildRowActions(entry, p, {
    startCreate: (parentDir, type) => setCreating({ parentDir, type }),
    startRename: (path, isDir) => setRenaming({ path, isDir }),
    startDelete: (path, isDir) => setConfirming({ path, isDir }),
  });

  return (
    <>
      {p.entries.map((entry) => renderNode(entry, 0, []))}
      {ctx && (
        <ExplorerContextMenu
          x={ctx.x} y={ctx.y}
          title={ctx.entry.name}
          actions={actionsFor(ctx.entry)}
          onClose={() => setCtx(null)}
        />
      )}
    </>
  );

  /** `rails` records, per ancestor level, whether that guide should be lit. */
  function renderNode(entry: FileEntry, depth: number, rails: boolean[]) {
    return entry.is_dir
      ? <DirNode key={entry.path} entry={entry} depth={depth} rails={rails} />
      : <FileNode key={entry.path} entry={entry} depth={depth} rails={rails} />;
  }

  function Rails({ rails }: { rails: boolean[] }) {
    if (rails.length === 0) return null;
    return (
      <span className="xpl-rails" aria-hidden="true">
        {rails.map((lit, i) => (
          <span key={i} className={`xpl-rail${lit ? " is-lit" : ""}`} style={{ left: `${8 + i * INDENT + 6}px` }} />
        ))}
      </span>
    );
  }

  function RowShell({ entry, children, className, onClick, depth }: {
    entry: FileEntry; children: React.ReactNode; className: string;
    onClick: () => void; depth: number;
  }) {
    return (
      <div
        className="xpl-row"
        onContextMenu={(e) => { e.preventDefault(); openCtx(entry, e.clientX, e.clientY); }}
      >
        <button
          type="button"
          className={className}
          style={{ paddingLeft: `${8 + depth * INDENT}px` }}
          onClick={onClick}
          onKeyDown={(e) => onRowKey(e, entry)}
          title={entry.path}
        >
          {children}
        </button>
        <button
          type="button"
          className="xpl-row-more"
          aria-label={`Actions for ${entry.name}`}
          onClick={(e) => {
            e.stopPropagation();
            const r = e.currentTarget.getBoundingClientRect();
            openCtx(entry, r.right - 4, r.bottom + 4);
          }}
        >
          <MoreVertical size={13} />
        </button>
      </div>
    );
  }

  function DirNode({ entry, depth, rails }: { entry: FileEntry; depth: number; rails: boolean[] }) {
    const isOpen = p.expandedDirs.has(entry.path);
    const children = p.dirChildren[entry.path] || [];
    const onChain = litPaths.has(entry.path);
    const childRails = [...rails, onChain];

    return (
      <div className="xpl-branch">
        <Rails rails={rails} />
        <RowShell
          entry={entry} depth={depth}
          className={`xpl-entry xpl-entry-dir${isOpen ? " is-open" : ""}${onChain ? " on-chain" : ""}`}
          onClick={() => p.toggleDir(entry.path)}
        >
          <DirTile open={isOpen} loading={p.loadingDirs.has(entry.path)} />
          {isOpen
            ? <FolderOpen size={14} className="xpl-folder-icon" />
            : <Folder size={14} className="xpl-folder-icon" />}
          <span className="xpl-name">{entry.name}</span>
        </RowShell>

        {renaming?.path === entry.path && (
          <InlineNameField
            initial={entry.name} depth={depth} icon={<Pencil size={12} className="xpl-field-icon" />}
            onSubmit={(name) => { p.onRename?.(entry.path, name, true); setRenaming(null); }}
            onCancel={() => setRenaming(null)}
          />
        )}
        {confirming?.path === entry.path && (
          <ConfirmDelete
            name={entry.name} isDir depth={depth}
            onConfirm={() => { p.onDeleteDir?.(entry.path); setConfirming(null); }}
            onCancel={() => setConfirming(null)}
          />
        )}
        {creating?.parentDir === entry.path && (
          <InlineNameField
            initial="" depth={depth + 1}
            placeholder={creating.type === "file" ? "filename.ext" : "folder name"}
            icon={creating.type === "dir"
              ? <FolderPlus size={12} className="xpl-field-icon" />
              : <FilePlus size={12} className="xpl-field-icon" />}
            onSubmit={(name) => {
              if (creating.type === "file") p.onCreateFile?.(entry.path, name);
              else p.onCreateDir?.(entry.path, name);
              setCreating(null);
            }}
            onCancel={() => setCreating(null)}
          />
        )}
        {isOpen && children.length > 0 && (
          <div className="xpl-children">
            {children.map((child) => renderNode(child, depth + 1, childRails))}
          </div>
        )}
        {isOpen && children.length === 0 && !p.loadingDirs.has(entry.path) && (
          <div className="xpl-empty-branch" style={{ paddingLeft: `${8 + (depth + 1) * INDENT}px` }}>
            Empty
          </div>
        )}
      </div>
    );
  }

  function FileNode({ entry, depth, rails }: { entry: FileEntry; depth: number; rails: boolean[] }) {
    const isActive = p.activeFilePath === entry.path;
    return (
      <div className="xpl-branch">
        <Rails rails={rails} />
        <RowShell
          entry={entry} depth={depth}
          className={`xpl-entry xpl-entry-file${isActive ? " is-active" : ""}`}
          onClick={() => p.onFileClick(entry.path)}
        >
          <FileTile name={entry.name} />
          <span className="xpl-name">{entry.name}</span>
        </RowShell>

        {renaming?.path === entry.path && (
          <InlineNameField
            initial={entry.name} selectStem depth={depth}
            icon={<Pencil size={12} className="xpl-field-icon" />}
            onSubmit={(name) => { p.onRename?.(entry.path, name, false); setRenaming(null); }}
            onCancel={() => setRenaming(null)}
          />
        )}
        {confirming?.path === entry.path && (
          <ConfirmDelete
            name={entry.name} isDir={false} depth={depth}
            onConfirm={() => { p.onDeleteFile?.(entry.path); setConfirming(null); }}
            onCancel={() => setConfirming(null)}
          />
        )}
      </div>
    );
  }
}
