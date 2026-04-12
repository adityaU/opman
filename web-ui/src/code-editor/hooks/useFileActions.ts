/**
 * useFileActions — file management actions (create, delete, rename, upload, reload).
 * Extracted from useFileExplorer to keep each hook under 300 lines.
 */
import { useState, useCallback } from "react";
import { browseFiles, createFile, createDir, deleteFile, deleteDir, uploadFiles, renameEntry, docRead, readFile, classifyFile, type FileEntry } from "../../api";
import type { OpenFileEntry, FileRenderType } from "../types";
import { isDocRenderType } from "../types";

export interface FileActionsState {
  handleCreateFile: (parentDir: string, name: string) => Promise<void>;
  handleCreateDir: (parentDir: string, name: string) => Promise<void>;
  handleDeleteFile: (filePath: string) => Promise<void>;
  handleDeleteDir: (dirPath: string) => Promise<void>;
  handleUploadFiles: (dir: string, files: FileList | File[]) => Promise<void>;
  handleRename: (oldPath: string, newName: string, isDir: boolean) => Promise<void>;
  handleReloadDir: (dirPath: string) => Promise<void>;
  handleReloadFile: (filePath: string) => Promise<void>;
  handleReloadRoot: () => Promise<void>;
  fileActionBusy: boolean;
}

interface Deps {
  currentPath: string;
  expandedDirs: Set<string>;
  openFiles: OpenFileEntry[];
  activeFilePath: string | null;
  loadDirectory: (path: string) => Promise<void>;
  closeFile: (path: string) => void;
  setEntries: React.Dispatch<React.SetStateAction<FileEntry[]>>;
  setCurrentPath: React.Dispatch<React.SetStateAction<string>>;
  setDirChildren: React.Dispatch<React.SetStateAction<Record<string, FileEntry[]>>>;
  setExpandedDirs: React.Dispatch<React.SetStateAction<Set<string>>>;
  setOpenFiles: React.Dispatch<React.SetStateAction<OpenFileEntry[]>>;
  setActiveFilePath: (p: string | null) => void;
  setSaveStatus: (s: "saved" | "modified" | null) => void;
  onError?: (msg: string) => void;
}

export function useFileActions(deps: Deps): FileActionsState {
  const [fileActionBusy, setFileActionBusy] = useState(false);

  const refreshDirSubtree = useCallback(async (dirPath: string) => {
    await deps.loadDirectory(deps.currentPath);
    if (dirPath !== "." && deps.expandedDirs.has(dirPath)) {
      try {
        const resp = await browseFiles(dirPath);
        deps.setDirChildren((prev) => ({ ...prev, [dirPath]: resp.entries }));
      } catch { /* ignore */ }
    }
    const parentDir = dirPath.includes("/") ? dirPath.substring(0, dirPath.lastIndexOf("/")) : ".";
    if (parentDir !== dirPath && deps.expandedDirs.has(parentDir)) {
      try {
        const resp = await browseFiles(parentDir);
        deps.setDirChildren((prev) => ({ ...prev, [parentDir]: resp.entries }));
      } catch { /* ignore */ }
    }
  }, [deps.currentPath, deps.expandedDirs, deps.loadDirectory, deps.setDirChildren]);

  const handleCreateFile = useCallback(async (parentDir: string, name: string) => {
    const fullPath = parentDir === "." ? name : `${parentDir}/${name}`;
    setFileActionBusy(true);
    try {
      await createFile(fullPath);
      await refreshDirSubtree(parentDir);
    } catch (err) {
      console.error("Failed to create file:", err);
      deps.onError?.("Failed to create file");
    } finally {
      setFileActionBusy(false);
    }
  }, [deps.onError, refreshDirSubtree]);

  const handleCreateDir = useCallback(async (parentDir: string, name: string) => {
    const fullPath = parentDir === "." ? name : `${parentDir}/${name}`;
    setFileActionBusy(true);
    try {
      await createDir(fullPath);
      await refreshDirSubtree(parentDir);
    } catch (err) {
      console.error("Failed to create directory:", err);
      deps.onError?.("Failed to create directory");
    } finally {
      setFileActionBusy(false);
    }
  }, [deps.onError, refreshDirSubtree]);

  const handleDeleteFile = useCallback(async (filePath: string) => {
    setFileActionBusy(true);
    try {
      await deleteFile(filePath);
      if (deps.openFiles.some((f) => f.path === filePath)) deps.closeFile(filePath);
      const parentDir = filePath.includes("/") ? filePath.substring(0, filePath.lastIndexOf("/")) : ".";
      await refreshDirSubtree(parentDir);
    } catch (err) {
      console.error("Failed to delete file:", err);
      deps.onError?.("Failed to delete file");
    } finally {
      setFileActionBusy(false);
    }
  }, [deps.onError, deps.openFiles, deps.closeFile, refreshDirSubtree]);

  const handleDeleteDir = useCallback(async (dirPath: string) => {
    setFileActionBusy(true);
    try {
      await deleteDir(dirPath);
      const toClose = deps.openFiles.filter((f) => f.path.startsWith(dirPath + "/") || f.path === dirPath);
      toClose.forEach((f) => deps.closeFile(f.path));
      deps.setExpandedDirs((prev) => { const next = new Set(prev); next.delete(dirPath); return next; });
      const parentDir = dirPath.includes("/") ? dirPath.substring(0, dirPath.lastIndexOf("/")) : ".";
      await refreshDirSubtree(parentDir);
    } catch (err) {
      console.error("Failed to delete directory:", err);
      deps.onError?.("Failed to delete directory");
    } finally {
      setFileActionBusy(false);
    }
  }, [deps.onError, deps.openFiles, deps.closeFile, deps.setExpandedDirs, refreshDirSubtree]);

  const handleUploadFiles = useCallback(async (dir: string, files: FileList | File[]) => {
    setFileActionBusy(true);
    try {
      await uploadFiles(dir, files);
      await refreshDirSubtree(dir);
    } catch (err) {
      console.error("Failed to upload files:", err);
      deps.onError?.("Failed to upload files");
    } finally {
      setFileActionBusy(false);
    }
  }, [deps.onError, refreshDirSubtree]);

  const handleRename = useCallback(async (oldPath: string, newName: string, isDir: boolean) => {
    const parentDir = oldPath.includes("/") ? oldPath.substring(0, oldPath.lastIndexOf("/")) : ".";
    const newPath = parentDir === "." ? newName : `${parentDir}/${newName}`;
    if (newPath === oldPath) return;
    setFileActionBusy(true);
    try {
      await renameEntry(oldPath, newPath);
      // Remap open file tabs
      deps.setOpenFiles((prev) => prev.map((f) => {
        if (isDir) {
          const prefix = oldPath + "/";
          if (f.path === oldPath || f.path.startsWith(prefix)) {
            return { ...f, path: newPath + f.path.substring(oldPath.length) };
          }
        } else if (f.path === oldPath) {
          return { ...f, path: newPath };
        }
        return f;
      }));
      // Remap active file path
      if (deps.activeFilePath) {
        if (isDir && (deps.activeFilePath === oldPath || deps.activeFilePath.startsWith(oldPath + "/"))) {
          deps.setActiveFilePath(newPath + deps.activeFilePath.substring(oldPath.length));
        } else if (!isDir && deps.activeFilePath === oldPath) {
          deps.setActiveFilePath(newPath);
        }
      }
      // Remap expanded dirs + dir children cache
      if (isDir) {
        deps.setExpandedDirs((prev) => {
          const next = new Set<string>();
          for (const d of prev) {
            if (d === oldPath) next.add(newPath);
            else if (d.startsWith(oldPath + "/")) next.add(newPath + d.substring(oldPath.length));
            else next.add(d);
          }
          return next;
        });
        deps.setDirChildren((prev) => {
          const next: Record<string, FileEntry[]> = {};
          for (const [k, v] of Object.entries(prev)) {
            if (k === oldPath) next[newPath] = v;
            else if (k.startsWith(oldPath + "/")) next[newPath + k.substring(oldPath.length)] = v;
            else next[k] = v;
          }
          return next;
        });
      }
      await refreshDirSubtree(parentDir);
    } catch (err) {
      console.error("Failed to rename:", err);
      deps.onError?.("Failed to rename");
    } finally {
      setFileActionBusy(false);
    }
  }, [deps.onError, deps.activeFilePath, deps.setOpenFiles, deps.setActiveFilePath, deps.setExpandedDirs, deps.setDirChildren, refreshDirSubtree]);

  const handleReloadDir = useCallback(async (dirPath: string) => {
    try {
      const resp = await browseFiles(dirPath === "." ? undefined : dirPath);
      if (dirPath === "." || dirPath === deps.currentPath) {
        deps.setEntries(resp.entries);
        deps.setCurrentPath(resp.path || ".");
      }
      if (dirPath !== "." && deps.expandedDirs.has(dirPath)) {
        deps.setDirChildren((prev) => ({ ...prev, [dirPath]: resp.entries }));
      }
    } catch (err) {
      console.error("Failed to reload directory:", err);
      deps.onError?.("Failed to reload directory");
    }
  }, [deps.currentPath, deps.expandedDirs, deps.onError, deps.setEntries, deps.setCurrentPath, deps.setDirChildren]);

  const handleReloadFile = useCallback(async (filePath: string) => {
    const existing = deps.openFiles.find((f) => f.path === filePath);
    if (!existing) return;
    const renderType: FileRenderType = classifyFile(filePath);
    if (renderType === "image" || renderType === "audio" || renderType === "video" || renderType === "pdf" || renderType === "model3d" || renderType === "binary") return;
    try {
      if (isDocRenderType(renderType)) {
        const resp = await docRead(filePath);
        deps.setOpenFiles((prev) =>
          prev.map((f) => f.path === filePath ? { ...f, docData: resp.data, editedDocData: null } : f),
        );
      } else {
        const resp = await readFile(filePath);
        deps.setOpenFiles((prev) =>
          prev.map((f) => f.path === filePath ? { ...f, content: resp.content, language: resp.language, editedContent: null } : f),
        );
      }
      if (filePath === deps.activeFilePath) deps.setSaveStatus(null);
    } catch (err) {
      console.error("Failed to reload file:", err);
      deps.onError?.("Failed to reload file");
    }
  }, [deps.openFiles, deps.activeFilePath, deps.onError, deps.setOpenFiles, deps.setSaveStatus]);

  const handleReloadRoot = useCallback(async () => {
    try {
      const resp = await browseFiles(deps.currentPath === "." ? undefined : deps.currentPath);
      deps.setEntries(resp.entries);
      deps.setCurrentPath(resp.path || ".");
      const expanded = Array.from(deps.expandedDirs);
      await Promise.all(expanded.map(async (dir) => {
        try {
          const r = await browseFiles(dir);
          deps.setDirChildren((prev) => ({ ...prev, [dir]: r.entries }));
        } catch { /* ignore individual failures */ }
      }));
    } catch (err) {
      console.error("Failed to reload root:", err);
      deps.onError?.("Failed to reload explorer");
    }
  }, [deps.currentPath, deps.expandedDirs, deps.onError, deps.setEntries, deps.setCurrentPath, deps.setDirChildren]);

  return {
    handleCreateFile, handleCreateDir, handleDeleteFile, handleDeleteDir,
    handleUploadFiles, handleRename, handleReloadDir, handleReloadFile, handleReloadRoot,
    fileActionBusy,
  };
}
