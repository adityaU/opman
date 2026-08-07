/**
 * ExplorerTree — the recursive file tree.
 *
 * Two decisions shape it. Rows carry no controls: actions open on right-click
 * or the row's single trailing button, so the panel spends its width on the
 * names it exists to show. And every row draws the indent guides for its
 * ancestors, with the guides along the active file's chain lit — the "spine" —
 * so the answer to "where am I" is visible without expanding anything.
 *
 * The row components sit at module scope rather than inside `ExplorerTree`.
 * Declared inline they were a *new component type* on every render, so React
 * unmounted and remounted the entire tree whenever anything above it changed —
 * including the keymap's own surface-focus context, which changes the instant a
 * row takes focus. Keyboard traversal was impossible: the first `j` moved focus
 * to a row, the surface flipped to `explorer`, the tree remounted, and focus
 * fell back to `<body>`. Everything the rows need is threaded through `TreeCtx`.
 */
import { useCallback, useMemo, useState } from "react";
import { Folder, FolderOpen, MoreVertical, FilePlus, FolderPlus, Pencil } from "lucide-react";
import type { FileEntry } from "../types";
import { ExplorerContextMenu } from "../explorer/ExplorerContextMenu";
import { buildRowActions, type RowActionHandlers } from "../explorer/rowActions";
import { useListNav } from "../../keybindings/useListNav";
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

/** Everything a row needs from the tree that owns it. */
interface TreeCtx {
  p: Props;
  /** Ancestor paths of the open file — the segments the spine lights. */
  litPaths: Set<string>;
  openCtx: (entry: FileEntry, x: number, y: number) => void;
  onRowKey: (event: React.KeyboardEvent, entry: FileEntry) => void;
  renaming: Pending | null;
  setRenaming: (pending: Pending | null) => void;
  confirming: Pending | null;
  setConfirming: (pending: Pending | null) => void;
  creating: Creating | null;
  setCreating: (creating: Creating | null) => void;
}

export function ExplorerTree(p: Props) {
  const [creating, setCreating] = useState<Creating | null>(null);
  const [confirming, setConfirming] = useState<Pending | null>(null);
  const [renaming, setRenaming] = useState<Pending | null>(null);
  const [ctx, setCtx] = useState<CtxTarget | null>(null);

  // Traversal for the whole tree, registered here because this is what exists
  // whenever there is a tree to walk. The defaults do the work: a row's click
  // already toggles a folder or opens a file, so expand, collapse and open need
  // no handlers of their own — only the keys, which the layers supply.
  useListNav({
    surface: "explorer",
    commands: {
      moveDown: "explorer.moveDown",
      moveUp: "explorer.moveUp",
      expand: "explorer.expand",
      collapse: "explorer.collapse",
      activate: "explorer.open",
    },
  });

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

  const tree: TreeCtx = {
    p, litPaths, openCtx, onRowKey,
    renaming, setRenaming, confirming, setConfirming, creating, setCreating,
  };

  return (
    <>
      {p.entries.map((entry) => (
        <TreeNode key={entry.path} tree={tree} entry={entry} depth={0} rails={[]} />
      ))}
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
}

interface NodeProps {
  tree: TreeCtx;
  entry: FileEntry;
  depth: number;
  /** Per ancestor level, whether that indent guide should be lit. */
  rails: boolean[];
}

function TreeNode({ tree, entry, depth, rails }: NodeProps) {
  return entry.is_dir
    ? <DirNode tree={tree} entry={entry} depth={depth} rails={rails} />
    : <FileNode tree={tree} entry={entry} depth={depth} rails={rails} />;
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

function RowShell({ tree, entry, children, className, onClick, depth, expanded }: {
  tree: TreeCtx; entry: FileEntry; children: React.ReactNode; className: string;
  onClick: () => void; depth: number;
  /** Absent on files — only a branch has an open/closed state to report. */
  expanded?: boolean;
}) {
  return (
    <div
      className="xpl-row"
      onContextMenu={(e) => { e.preventDefault(); tree.openCtx(entry, e.clientX, e.clientY); }}
    >
      <button
        type="button"
        className={className}
        style={{ paddingLeft: `${8 + depth * INDENT}px` }}
        onClick={onClick}
        onKeyDown={(e) => tree.onRowKey(e, entry)}
        title={entry.path}
        // What `useListNav` traverses. Depth is what lets `h` on a leaf climb to
        // its folder, `aria-expanded` is what tells `h`/`l` whether this row has
        // a branch to open at all, and the key is the identity focus is restored
        // by when opening a folder rebuilds the branch under it.
        data-list-item=""
        data-list-key={entry.path}
        data-list-depth={depth}
        aria-expanded={expanded}
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
          tree.openCtx(entry, r.right - 4, r.bottom + 4);
        }}
      >
        <MoreVertical size={13} />
      </button>
    </div>
  );
}

function DirNode({ tree, entry, depth, rails }: NodeProps) {
  const { p, renaming, confirming, creating } = tree;
  const isOpen = p.expandedDirs.has(entry.path);
  const children = p.dirChildren[entry.path] || [];
  const onChain = tree.litPaths.has(entry.path);
  const childRails = [...rails, onChain];

  return (
    <div className="xpl-branch">
      <Rails rails={rails} />
      <RowShell
        tree={tree} entry={entry} depth={depth} expanded={isOpen}
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
          onSubmit={(name) => { p.onRename?.(entry.path, name, true); tree.setRenaming(null); }}
          onCancel={() => tree.setRenaming(null)}
        />
      )}
      {confirming?.path === entry.path && (
        <ConfirmDelete
          name={entry.name} isDir depth={depth}
          onConfirm={() => { p.onDeleteDir?.(entry.path); tree.setConfirming(null); }}
          onCancel={() => tree.setConfirming(null)}
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
            tree.setCreating(null);
          }}
          onCancel={() => tree.setCreating(null)}
        />
      )}
      {isOpen && children.length > 0 && (
        <div className="xpl-children">
          {children.map((child) => (
            <TreeNode key={child.path} tree={tree} entry={child} depth={depth + 1} rails={childRails} />
          ))}
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

function FileNode({ tree, entry, depth, rails }: NodeProps) {
  const { p, renaming, confirming } = tree;
  const isActive = p.activeFilePath === entry.path;
  return (
    <div className="xpl-branch">
      <Rails rails={rails} />
      <RowShell
        tree={tree} entry={entry} depth={depth}
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
          onSubmit={(name) => { p.onRename?.(entry.path, name, false); tree.setRenaming(null); }}
          onCancel={() => tree.setRenaming(null)}
        />
      )}
      {confirming?.path === entry.path && (
        <ConfirmDelete
          name={entry.name} isDir={false} depth={depth}
          onConfirm={() => { p.onDeleteFile?.(entry.path); tree.setConfirming(null); }}
          onCancel={() => tree.setConfirming(null)}
        />
      )}
    </div>
  );
}
