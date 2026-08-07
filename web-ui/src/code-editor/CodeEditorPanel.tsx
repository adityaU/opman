/**
 * CodeEditorPanel — orchestrator component.
 *
 * Wires together the three domain hooks (file explorer, file editor, LSP)
 * and delegates rendering to DesktopLayout or MobileLayout.
 */
import { useEffect, useCallback, useMemo, useRef } from "react";
import type { CodeEditorPanelProps, FileRenderType } from "./types";
import type { FileReadResponse } from "./types";
import { fileDownloadUrl, dirDownloadUrl, triggerDownload } from "../api";
import { useFileExplorer } from "./hooks/useFileExplorer";
import { useFileEditor } from "./hooks/useFileEditor";
import { useLspFeatures } from "./hooks/useLspFeatures";
import { useEditorCommands } from "./useEditorCommands";
import { useEditorLsp } from "./hooks/useEditorLsp";
import { DesktopLayout } from "./components/DesktopLayout";
import { MobileLayout } from "./components/MobileLayout";

export default function CodeEditorPanel({
  focused, openFilePath, openLine, projectPath, sessionId, onError, layout,
}: CodeEditorPanelProps) {
  const explorer = useFileExplorer(projectPath, openFilePath, openLine, onError);
  const editorRef = useRef<HTMLDivElement>(null);

  // Derive the active open file entry
  const activeEntry = explorer.openFiles.find((f) => f.path === explorer.activeFilePath) ?? null;

  const editor = useFileEditor(explorer.activeFilePath, activeEntry);

  const openFile: FileReadResponse | null = activeEntry
    ? { path: activeEntry.path, content: activeEntry.content, language: activeEntry.language }
    : null;
  const isModified = explorer.editedContent !== null;
  const currentContent = isModified ? explorer.editedContent! : openFile?.content ?? "";
  const fileRenderType: FileRenderType = activeEntry?.renderType ?? "code";

  const lsp = useLspFeatures(
    activeEntry, explorer.activeFilePath, sessionId,
    currentContent, editor.cursorLine, editor.cursorCol,
    explorer.loadFile, explorer.setOpenFiles, explorer.setSaveStatus,
    onError,
  );

  // LSP lives inside the editor: hover tooltips, ⌘-click / F12 to definition,
  // ⇧⌥F to format, diagnostics underlined in place.
  const lspExtensions = useEditorLsp({
    enabled: lsp.lspAvailable,
    activeFilePath: explorer.activeFilePath,
    activeDiagnostics: lsp.activeDiagnostics,
    editorViewRef: editor.editorViewRef,
    hoverAt: lsp.hoverAt,
    definitionAt: lsp.definitionAt,
    format: lsp.handleFormatWithLsp,
    completeAt: lsp.completeAt,
    triggerCharacters: lsp.triggerCharacters,
  });

  const extensions = useMemo(
    () => [...editor.extensions, ...lspExtensions],
    [editor.extensions, lspExtensions],
  );

  useEditorCommands({
    hasOpenFile: Boolean(openFile),
    save: explorer.handleSave,
    revert: explorer.handleRevert,
    format: lsp.handleFormatWithLsp,
    goToDefinition: lsp.handleDefinition,
    hover: lsp.handleHover,
    reloadRoot: explorer.handleReloadRoot,
  });

  // Pending line jump
  useEffect(() => {
    const pending = explorer.pendingJumpRef.current;
    if (!pending || pending.path !== explorer.activeFilePath || editor.activeView !== "code") return;
    editor.jumpToLine(pending.line);
    explorer.pendingJumpRef.current = null;
  }, [explorer.activeFilePath, editor.activeView, editor.jumpToLine, currentContent]);

  // CodeMirror callbacks
  const onCreateEditor = useCallback((view: any) => {
    editor.editorViewRef.current = view;
  }, [editor.editorViewRef]);

  const onUpdate = useCallback((update: any) => {
    const pos = update.state.selection.main.head;
    const line = update.state.doc.lineAt(pos);
    editor.setCursorLine(line.number);
    editor.setCursorCol(pos - line.from + 1);
  }, [editor.setCursorLine, editor.setCursorCol]);

  // Clicking a diagnostic moves the cursor to it and focuses the editor.
  const handleJumpToLine = useCallback((line: number, _col: number) => {
    editor.jumpToLine(line);
  }, [editor.jumpToLine]);

  // Mobile navigation helpers
  const handleEntryClick = useCallback((entry: { is_dir: boolean; path: string }) => {
    if (entry.is_dir) explorer.loadDirectory(entry.path);
    else explorer.loadFile(entry.path);
  }, [explorer.loadDirectory, explorer.loadFile]);

  const handleBackToBrowser = useCallback(() => {
    explorer.setActiveFilePath(null);
    explorer.setSaveStatus(null);
  }, [explorer.setActiveFilePath, explorer.setSaveStatus]);

  // Download helpers
  const handleDownloadFile = useCallback((path: string) => {
    triggerDownload(fileDownloadUrl(path));
  }, []);

  const handleDownloadDir = useCallback((path: string) => {
    triggerDownload(dirDownloadUrl(path));
  }, []);

  // Shared props for both layouts
  const shared = {
    editorRef,
    openFile,
    fileRenderType,
    isModified,
    currentContent,
    activeView: editor.activeView,
    setActiveView: editor.setActiveView,
    extensions,
    onEditorChange: explorer.onEditorChange,
    onCreateEditor,
    onUpdate,
    loadingFile: explorer.loadingFile,
    languageLoading: editor.languageLoading,
    lspAvailable: lsp.lspAvailable,
    lspBusy: lsp.lspBusy,
    activeDiagnostics: lsp.activeDiagnostics,
    hoverText: lsp.hoverText,
    handleHover: lsp.handleHover,
    handleDefinition: lsp.handleDefinition,
    handleFormatWithLsp: lsp.handleFormatWithLsp,
    onJumpToLine: handleJumpToLine,
    saveStatus: explorer.saveStatus,
    saving: explorer.saving,
    handleSave: explorer.handleSave,
    handleRevert: explorer.handleRevert,
    // Doc-type editing
    activeEntry,
    setOpenFiles: explorer.setOpenFiles,
    // File management
    onCreateFile: explorer.handleCreateFile,
    onCreateDir: explorer.handleCreateDir,
    onDeleteFile: explorer.handleDeleteFile,
    onDeleteDir: explorer.handleDeleteDir,
    onUploadFiles: explorer.handleUploadFiles,
    onReloadDir: explorer.handleReloadDir,
    onReloadFile: explorer.handleReloadFile,
    onReloadRoot: explorer.handleReloadRoot,
    onRename: explorer.handleRename,
    onDownloadFile: handleDownloadFile,
    onDownloadDir: handleDownloadDir,
    fileActionBusy: explorer.fileActionBusy,
  };

  // An explicit `layout` wins: the mount site knows which shell it lives in,
  // and trusting the viewport instead is what let the mobile sheet render the
  // desktop editor.
  const isDesktop = layout ? layout === "desktop" : editor.isDesktop;

  if (isDesktop) {
    return (
      <DesktopLayout
        {...shared}
        explorerCollapsed={explorer.explorerCollapsed}
        setExplorerCollapsed={explorer.setExplorerCollapsed}
        entries={explorer.entries}
        loadingDir={explorer.loadingDir}
        expandedDirs={explorer.expandedDirs}
        dirChildren={explorer.dirChildren}
        loadingDirs={explorer.loadingDirs}
        toggleDir={explorer.toggleDir}
        openFiles={explorer.openFiles}
        activeFilePath={explorer.activeFilePath}
        setActiveFilePath={explorer.setActiveFilePath}
        setSaveStatus={explorer.setSaveStatus}
        closeFile={explorer.closeFile}
        loadFile={explorer.loadFile}
        currentPath={explorer.currentPath}
      />
    );
  }

  return (
    <MobileLayout
      {...shared}
      breadcrumbs={explorer.breadcrumbs}
      entries={explorer.entries}
      loadingDir={explorer.loadingDir}
      loadDirectory={explorer.loadDirectory}
      loadFile={explorer.loadFile}
      onEntryClick={handleEntryClick}
      onBackToBrowser={handleBackToBrowser}
      currentPath={explorer.currentPath}
    />
  );
}
