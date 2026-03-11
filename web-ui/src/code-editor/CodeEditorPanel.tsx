/**
 * CodeEditorPanel — orchestrator component.
 *
 * Wires together the three domain hooks (file explorer, file editor, LSP)
 * and delegates rendering to DesktopLayout or MobileLayout.
 */
import { useEffect, useCallback, useRef } from "react";
import type { CodeEditorPanelProps, FileRenderType } from "./types";
import type { FileReadResponse } from "./types";
import { useFileExplorer } from "./hooks/useFileExplorer";
import { useFileEditor } from "./hooks/useFileEditor";
import { useLspFeatures } from "./hooks/useLspFeatures";
import { DesktopLayout } from "./components/DesktopLayout";
import { MobileLayout } from "./components/MobileLayout";

export default function CodeEditorPanel({
  focused, openFilePath, openLine, projectPath, sessionId, onError,
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

  // Keyboard shortcut: Cmd+S / Ctrl+S
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s" && openFile && focused) {
        e.preventDefault();
        explorer.handleSave();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [explorer.handleSave, openFile, focused]);

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

  // Mobile navigation helpers
  const handleEntryClick = useCallback((entry: { is_dir: boolean; path: string }) => {
    if (entry.is_dir) explorer.loadDirectory(entry.path);
    else explorer.loadFile(entry.path);
  }, [explorer.loadDirectory, explorer.loadFile]);

  const handleBackToBrowser = useCallback(() => {
    explorer.setActiveFilePath(null);
    explorer.setSaveStatus(null);
  }, [explorer.setActiveFilePath, explorer.setSaveStatus]);

  // Shared props for both layouts
  const shared = {
    editorRef,
    openFile,
    fileRenderType,
    isModified,
    currentContent,
    activeView: editor.activeView,
    setActiveView: editor.setActiveView,
    extensions: editor.extensions,
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
    saveStatus: explorer.saveStatus,
    saving: explorer.saving,
    handleSave: explorer.handleSave,
    handleRevert: explorer.handleRevert,
  };

  if (editor.isDesktop) {
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
      onEntryClick={handleEntryClick}
      onBackToBrowser={handleBackToBrowser}
    />
  );
}
