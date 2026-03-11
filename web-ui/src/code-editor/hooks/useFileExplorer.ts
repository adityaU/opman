import { useState, useCallback, useEffect, useRef } from "react";
import { browseFiles, readFile, classifyFile, type FileEntry } from "../../api";
import type { OpenFileEntry, BreadcrumbEntry, FileRenderType } from "../types";

export interface FileExplorerState {
  // Directory browsing
  currentPath: string;
  entries: FileEntry[];
  loadingDir: boolean;
  explorerCollapsed: boolean;
  setExplorerCollapsed: (v: boolean) => void;
  loadDirectory: (path: string) => Promise<void>;
  // Tree expansion (desktop)
  expandedDirs: Set<string>;
  dirChildren: Record<string, FileEntry[]>;
  loadingDirs: Set<string>;
  toggleDir: (dirPath: string) => void;
  // Open file management
  openFiles: OpenFileEntry[];
  activeFilePath: string | null;
  setActiveFilePath: (p: string | null) => void;
  loadingFile: boolean;
  loadFile: (path: string, line?: number | null) => Promise<void>;
  closeFile: (path: string) => void;
  // Save state
  saveStatus: "saved" | "modified" | null;
  setSaveStatus: (s: "saved" | "modified" | null) => void;
  saving: boolean;
  handleSave: () => Promise<void>;
  handleRevert: () => void;
  // Content editing
  editedContent: string | null;
  setOpenFiles: React.Dispatch<React.SetStateAction<OpenFileEntry[]>>;
  onEditorChange: (value: string) => void;
  // Breadcrumbs (mobile)
  breadcrumbs: BreadcrumbEntry[];
  // Jump to line support
  pendingJumpRef: React.MutableRefObject<{ path: string; line: number } | null>;
}

export function useFileExplorer(
  projectPath: string | null | undefined,
  openFilePath: string | null | undefined,
  openLine: number | null | undefined,
  onError?: (msg: string) => void,
): FileExplorerState {
  const [currentPath, setCurrentPath]         = useState(".");
  const [entries, setEntries]                 = useState<FileEntry[]>([]);
  const [loadingDir, setLoadingDir]           = useState(false);
  const [explorerCollapsed, setExplorerCollapsed] = useState(false);

  const [expandedDirs, setExpandedDirs]       = useState<Set<string>>(new Set());
  const [dirChildren, setDirChildren]         = useState<Record<string, FileEntry[]>>({});
  const [loadingDirs, setLoadingDirs]         = useState<Set<string>>(new Set());

  const [openFiles, setOpenFiles]             = useState<OpenFileEntry[]>([]);
  const [activeFilePath, setActiveFilePath]   = useState<string | null>(null);
  const [loadingFile, setLoadingFile]         = useState(false);
  const [saving, setSaving]                   = useState(false);
  const [saveStatus, setSaveStatus]           = useState<"saved" | "modified" | null>(null);

  const pendingJumpRef = useRef<{ path: string; line: number } | null>(null);

  // Derived
  const activeEntry = openFiles.find((f) => f.path === activeFilePath) ?? null;
  const editedContent = activeEntry?.editedContent ?? null;

  // Breadcrumbs (mobile)
  const breadcrumbs: BreadcrumbEntry[] = (() => {
    const parts = currentPath === "." ? [] : currentPath.split("/");
    const crumbs: BreadcrumbEntry[] = [{ path: ".", label: "root" }];
    for (let i = 0; i < parts.length; i++) {
      crumbs.push({ path: parts.slice(0, i + 1).join("/"), label: parts[i] });
    }
    return crumbs;
  })();

  // ── Directory loading ─────────────────────────────────

  const loadDirectory = useCallback(async (path: string) => {
    setLoadingDir(true);
    try {
      const resp = await browseFiles(path === "." ? undefined : path);
      setEntries(resp.entries);
      setCurrentPath(resp.path || ".");
    } catch (err) {
      console.error("Failed to browse files:", err);
      onError?.("Failed to load directory");
      setEntries([]);
    } finally {
      setLoadingDir(false);
    }
  }, [onError]);

  const loadDirChildren = useCallback(async (dirPath: string) => {
    setLoadingDirs((prev) => new Set([...prev, dirPath]));
    try {
      const resp = await browseFiles(dirPath);
      setDirChildren((prev) => ({ ...prev, [dirPath]: resp.entries }));
    } catch (err) {
      console.error("Failed to browse directory:", err);
      onError?.("Failed to expand directory");
    } finally {
      setLoadingDirs((prev) => { const next = new Set(prev); next.delete(dirPath); return next; });
    }
  }, [onError]);

  const toggleDir = useCallback((dirPath: string) => {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(dirPath)) {
        next.delete(dirPath);
      } else {
        next.add(dirPath);
        if (!dirChildren[dirPath]) loadDirChildren(dirPath);
      }
      return next;
    });
  }, [dirChildren, loadDirChildren]);

  // ── File loading ──────────────────────────────────────

  const loadFile = useCallback(async (path: string, line?: number | null) => {
    const existing = openFiles.find((f) => f.path === path);
    if (existing) {
      setActiveFilePath(path);
      setSaveStatus(existing.editedContent !== null ? "modified" : null);
      if (line) pendingJumpRef.current = { path, line };
      return;
    }

    const renderType = classifyFile(path);
    setLoadingFile(true);
    try {
      let content = "";
      let language: string = renderType;
      if (renderType !== "image" && renderType !== "audio" && renderType !== "video" && renderType !== "pdf" && renderType !== "binary") {
        const resp = await readFile(path);
        content = resp.content;
        language = resp.language;
      }
      const entry: OpenFileEntry = { path, content, language, renderType, editedContent: null };
      setOpenFiles((prev) => [...prev, entry]);
      setActiveFilePath(path);
      setSaveStatus(null);
      if (line) pendingJumpRef.current = { path, line };
    } catch (err) {
      console.error("Failed to read file:", err);
      onError?.("Failed to read file");
    } finally {
      setLoadingFile(false);
    }
  }, [openFiles, onError]);

  const closeFile = useCallback((path: string) => {
    setOpenFiles((prev) => {
      const idx = prev.findIndex((f) => f.path === path);
      if (idx === -1) return prev;
      const next = prev.filter((f) => f.path !== path);
      if (path === activeFilePath) {
        if (next.length === 0) { setActiveFilePath(null); setSaveStatus(null); }
        else {
          const neighbor = next[Math.min(idx, next.length - 1)];
          setActiveFilePath(neighbor.path);
          setSaveStatus(neighbor.editedContent !== null ? "modified" : null);
        }
      }
      return next;
    });
  }, [activeFilePath]);

  // ── Save / revert ─────────────────────────────────────

  const handleSave = useCallback(async () => {
    if (!activeFilePath || editedContent === null) return;
    const { writeFile } = await import("../../api");
    setSaving(true);
    try {
      await writeFile(activeFilePath, editedContent);
      setOpenFiles((prev) =>
        prev.map((f) => f.path === activeFilePath ? { ...f, content: editedContent, editedContent: null } : f),
      );
      setSaveStatus("saved");
      setTimeout(() => setSaveStatus(null), 2000);
    } catch (err) {
      console.error("Failed to save file:", err);
      onError?.("Failed to save file");
    } finally {
      setSaving(false);
    }
  }, [activeFilePath, editedContent, onError]);

  const handleRevert = useCallback(() => {
    if (!activeFilePath) return;
    setOpenFiles((prev) => prev.map((f) => f.path === activeFilePath ? { ...f, editedContent: null } : f));
    setSaveStatus(null);
  }, [activeFilePath]);

  // ── Content change handler ────────────────────────────

  const onEditorChange = useCallback((value: string) => {
    if (!activeFilePath || !activeEntry) return;
    if (value !== activeEntry.content) {
      setOpenFiles((prev) => prev.map((f) => f.path === activeFilePath ? { ...f, editedContent: value } : f));
      setSaveStatus("modified");
    } else {
      setOpenFiles((prev) => prev.map((f) => f.path === activeFilePath ? { ...f, editedContent: null } : f));
      setSaveStatus(null);
    }
  }, [activeFilePath, activeEntry]);

  // ── Effects ───────────────────────────────────────────

  useEffect(() => { loadDirectory("."); }, [loadDirectory]);

  // Project change reset
  const prevProjectPath = useRef(projectPath);
  useEffect(() => {
    if (projectPath === prevProjectPath.current) return;
    prevProjectPath.current = projectPath;
    setCurrentPath("."); setEntries([]);
    setExpandedDirs(new Set()); setDirChildren({});
    setOpenFiles([]); setActiveFilePath(null); setSaveStatus(null);
    loadDirectory(".");
  }, [projectPath, loadDirectory]);

  // External file open
  useEffect(() => {
    if (openFilePath) loadFile(openFilePath, openLine ?? undefined);
  }, [openFilePath, openLine, loadFile]);

  return {
    currentPath, entries, loadingDir,
    explorerCollapsed, setExplorerCollapsed, loadDirectory,
    expandedDirs, dirChildren, loadingDirs, toggleDir,
    openFiles, activeFilePath, setActiveFilePath,
    loadingFile, loadFile, closeFile,
    saveStatus, setSaveStatus, saving, handleSave, handleRevert,
    editedContent, setOpenFiles, onEditorChange,
    breadcrumbs, pendingJumpRef,
  };
}
