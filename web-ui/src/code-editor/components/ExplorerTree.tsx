/**
 * ExplorerTree — recursive file tree used in the desktop explorer sidebar.
 */
import { Folder, File, ChevronRight, ChevronDown, Loader2 } from "lucide-react";
import type { FileEntry } from "../types";

interface Props {
  entries: FileEntry[];
  expandedDirs: Set<string>;
  dirChildren: Record<string, FileEntry[]>;
  loadingDirs: Set<string>;
  activeFilePath: string | null;
  toggleDir: (dirPath: string) => void;
  onFileClick: (path: string) => void;
}

export function ExplorerTree({
  entries, expandedDirs, dirChildren, loadingDirs,
  activeFilePath, toggleDir, onFileClick,
}: Props) {
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

    return (
      <div>
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
    return (
      <button
        className={`explorer-tree-entry explorer-tree-file ${isActive ? "active" : ""}`}
        style={{ paddingLeft: `${8 + depth * 14 + 14}px` }}
        onClick={() => onFileClick(entry.path)}
      >
        <File size={14} className="file-icon" />
        <span className="file-name">{entry.name}</span>
      </button>
    );
  }
}
