/**
 * CodeEditorPanel — orchestrator component.
 *
 * Wires together the three domain hooks (file explorer, file editor, LSP)
 * and delegates rendering to DesktopLayout or MobileLayout.
 */
import { useEffect, useCallback, useMemo, useRef } from "react";
import type { EditorView, ViewUpdate } from "@codemirror/view";
import type { CodeEditorPanelProps, FileRenderType } from "./types";
import type { FileReadResponse } from "./types";
import { fileDownloadUrl, dirDownloadUrl, triggerDownload } from "../api";
import { useFileExplorer } from "./hooks/useFileExplorer";
import { useFileEditor } from "./hooks/useFileEditor";
import { useLspFeatures } from "./hooks/useLspFeatures";
import { useEditorCommands } from "./useEditorCommands";
import { useWhenContext } from "../keybindings/useCommand";
import { useEditorLsp } from "./hooks/useEditorLsp";
import { DesktopLayout } from "./components/DesktopLayout";
import { MobileLayout } from "./components/MobileLayout";

export default function CodeEditorPanel({
  focused, open, projectPath, sessionId, onError, layout,
}: CodeEditorPanelProps) {
  const explorer = useFileExplorer(projectPath, open, onError);
  const editorRef = useRef<HTMLDivElement>(null);

  // Derive the active open file entry
  const activeEntry = explorer.openFiles.find((f) => f.path === explorer.activeFilePath) ?? null;

  const editor = useFileEditor(explorer.activeFilePath, activeEntry);
  const isDesktop = layout ? layout === "desktop" : editor.isDesktop;

  const openFile: FileReadResponse | null = activeEntry
    ? { path: activeEntry.path, content: activeEntry.content, language: activeEntry.language }
    : null;
  const isModified = explorer.editedContent !== null;
  const currentContent = isModified ? explorer.editedContent! : openFile?.content ?? "";
  const fileRenderType: FileRenderType = activeEntry?.renderType ?? "code";

  const lsp = useLspFeatures(
    activeEntry, explorer.activeFilePath, sessionId, projectPath,
    currentContent, editor.cursorLine, editor.cursorCol,
    explorer.loadFile, explorer.setOpenFiles, explorer.setSaveStatus,
    onError,
  );

  // LSP lives inside the editor: hover tooltips, ⌘-click / F12 to definition,
  // ⇧⌥F to format, diagnostics underlined in place.
  const { extensions: lspExtensions } = useEditorLsp({
    enabled: lsp.lspAvailable,
    activeFilePath: explorer.activeFilePath,
    activeDiagnostics: lsp.activeDiagnostics,
    editorViewRef: editor.editorViewRef,
    hoverAt: lsp.hoverAt,
    definitionAt: lsp.definitionAt,
    format: lsp.handleFormatWithLsp,
    completeAt: lsp.completeAt,
    triggerCharacters: lsp.triggerCharacters,
    referencesAt: lsp.referencesAt,
    renameAt: lsp.renameAt,
    jumpTo: lsp.jumpTo,
    gotoAt: lsp.gotoAt,
    reveal: lsp.reveal,
  });

  const extensions = useMemo(
    () => [...editor.extensions, ...lspExtensions],
    [editor.extensions, lspExtensions],
  );

  useEditorCommands({
    hasOpenFile: Boolean(openFile),
    viewRef: editor.editorViewRef,
    save: explorer.handleSave,
    revert: explorer.handleRevert,
    format: lsp.handleFormatWithLsp,
    goToDefinition: lsp.handleDefinition,
    hover: lsp.handleHover,
    reloadRoot: explorer.handleReloadRoot,
  });

  // `mod+s` is scoped `when: editorDirty`, so an unpublished key left Save
  // inert: the binding never matched and the keystroke fell through.
  useWhenContext({
    editorDirty: explorer.saveStatus === "modified",
    anyDirty: explorer.openFiles.some((file) => file.editedContent !== null),
  });

  // Pending line jump
  useEffect(() => {
    const pending = explorer.pendingJumpRef.current;
    if (!pending || pending.path !== explorer.activeFilePath || editor.activeView !== "code") return;
    editor.jumpToLine(pending.line);
    explorer.pendingJumpRef.current = null;
  }, [explorer.activeFilePath, explorer.pendingJumpVersion, editor.activeView, editor.jumpToLine, currentContent]);

  // CodeMirror callbacks
  const onCreateEditor = useCallback((view: EditorView) => {
    editor.editorViewRef.current = view;
    view.requestMeasure();
  }, [editor.editorViewRef]);

  // The desktop explorer is a flex sibling, so dragging it changes the
  // editor's width without changing the panel's own box. Keep CodeMirror's
  // line wrapping and sticky gutter in sync with that sibling reflow.
  useEffect(() => {
    if (!isDesktop || typeof ResizeObserver === "undefined") return;
    const panel = editorRef.current;
    const editorMain = panel?.querySelector<HTMLElement>(".code-editor-main");
    if (!editorMain) return;

    // 0x0 means the pane's window is in the background and its contents are
    // being skipped, not that the editor got narrower.
    const resizeObserver = new ResizeObserver(() => {
      if (editorMain.clientWidth === 0) return;
      editor.editorViewRef.current?.requestMeasure();
    });
    resizeObserver.observe(editorMain);
    return () => resizeObserver.disconnect();
  }, [editor.editorViewRef, isDesktop]);

  const onUpdate = useCallback((update: ViewUpdate) => {
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
