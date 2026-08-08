/**
 * rowActions — builds the context-menu action list for one tree row.
 *
 * Kept out of the tree component so the tree stays about rendering, and so the
 * order of actions (create, rename, reload, download, then the destructive one
 * last and set apart) lives in exactly one place.
 */
import {
  FilePlus, FolderPlus, Trash2, RefreshCw, Pencil, Download,
} from "lucide-react";
import type { FileEntry } from "../types";
import type { MenuAction } from "./ExplorerContextMenu";

export interface RowActionHandlers {
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

interface Callbacks {
  startCreate: (parentDir: string, type: "file" | "dir") => void;
  startRename: (path: string, isDir: boolean) => void;
  startDelete: (path: string, isDir: boolean) => void;
}

export function buildRowActions(
  entry: FileEntry, h: RowActionHandlers, cb: Callbacks,
): MenuAction[] {
  const list: MenuAction[] = [];
  const dir = entry.is_dir;

  if (dir && h.onCreateFile) {
    list.push({
      key: "new-file", label: "New file", icon: <FilePlus size={13} />,
      command: "explorer.newFile",
      run: () => cb.startCreate(entry.path, "file"),
    });
  }
  if (dir && h.onCreateDir) {
    list.push({
      key: "new-dir", label: "New folder", icon: <FolderPlus size={13} />,
      command: "explorer.newFolder",
      run: () => cb.startCreate(entry.path, "dir"),
    });
  }
  if (h.onRename) {
    list.push({
      key: "rename", label: "Rename", icon: <Pencil size={13} />,
      command: "explorer.rename",
      run: () => cb.startRename(entry.path, dir),
    });
  }

  const reload = dir ? h.onReloadDir : h.onReloadFile;
  if (reload) {
    list.push({
      key: "reload", label: dir ? "Reload folder" : "Reload file",
      icon: <RefreshCw size={13} />, command: "explorer.reload",
      run: () => reload(entry.path),
    });
  }

  const download = dir ? h.onDownloadDir : h.onDownloadFile;
  if (download) {
    list.push({
      key: "download", label: dir ? "Download as zip" : "Download",
      icon: <Download size={13} />, command: "explorer.download",
      run: () => download(entry.path),
    });
  }

  const remove = dir ? h.onDeleteDir : h.onDeleteFile;
  if (remove) {
    list.push({
      key: "delete", label: dir ? "Delete folder" : "Delete file",
      icon: <Trash2 size={13} />, danger: true,
      command: "explorer.delete",
      run: () => cb.startDelete(entry.path, dir),
    });
  }

  return list;
}
