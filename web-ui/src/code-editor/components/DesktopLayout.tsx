/**
 * DesktopLayout — side-by-side file explorer + editor for desktop breakpoints.
 */
import { useRef, useState, useCallback } from "react";
import { Loader2, PanelLeftOpen, FilePlus, FolderPlus } from "lucide-react";
import type { OpenFileEntry, FileReadResponse, FileRenderType, EditorLspDiagnostic, EditorViewMode, FileEntry } from "../types";
import { EditorToolbar } from "./EditorToolbar";
import { EditorBody } from "./EditorBody";
import { ExplorerTree } from "./ExplorerTree";
import { ExplorerHeader } from "./ExplorerHeader";
import { ExplorerOpenFiles } from "../explorer/ExplorerOpenFiles";
import { ExplorerResults } from "../explorer/ExplorerResults";
import { InlineNameField } from "../explorer/ExplorerBits";
import { useExplorerFinder } from "../explorer/useExplorerFinder";
import { useExplorerChrome } from "../explorer/useExplorerChrome";
import { useCommands } from "../../keybindings/useCommand";

interface Props {
  editorRef: React.RefObject<HTMLDivElement>;
  // Explorer
  explorerCollapsed: boolean;
  setExplorerCollapsed: (v: boolean) => void;
  entries: FileEntry[];
  loadingDir: boolean;
  expandedDirs: Set<string>;
  dirChildren: Record<string, FileEntry[]>;
  loadingDirs: Set<string>;
  toggleDir: (dirPath: string) => void;
  currentPath: string;
  // Open files
  openFiles: OpenFileEntry[];
  activeFilePath: string | null;
  setActiveFilePath: (p: string | null) => void;
  setSaveStatus: (s: "saved" | "modified" | null) => void;
  closeFile: (path: string) => void;
  loadFile: (path: string, line?: number | null) => Promise<void>;
  // Active file
  openFile: FileReadResponse | null;
  fileRenderType: FileRenderType;
  isModified: boolean;
  currentContent: string;
  activeView: EditorViewMode;
  setActiveView: (mode: EditorViewMode) => void;
  // Editor
  extensions: any[];
  onEditorChange: (value: string) => void;
  onCreateEditor: (view: any) => void;
  onUpdate: (update: any) => void;
  loadingFile: boolean;
  languageLoading: boolean;
  // LSP
  lspAvailable: boolean;
  lspBusy: null | "hover" | "definition" | "format";
  activeDiagnostics: EditorLspDiagnostic[];
  hoverText: string | null;
  handleHover: () => void;
  handleDefinition: () => void;
  handleFormatWithLsp: () => void;
  onJumpToLine?: (line: number, col: number) => void;
  // Save
  saveStatus: "saved" | "modified" | null;
  saving: boolean;
  handleSave: () => void;
  handleRevert: () => void;
  // File management
  onCreateFile?: (parentDir: string, name: string) => void;
  onCreateDir?: (parentDir: string, name: string) => void;
  onDeleteFile?: (path: string) => void;
  onDeleteDir?: (path: string) => void;
  onUploadFiles?: (dir: string, files: FileList | File[]) => void;
  onReloadDir?: (dirPath: string) => void;
  onReloadFile?: (filePath: string) => void;
  onReloadRoot?: () => void;
  onRename?: (oldPath: string, newName: string, isDir: boolean) => void;
  onDownloadFile?: (path: string) => void;
  onDownloadDir?: (path: string) => void;
  fileActionBusy?: boolean;
  // Doc-type editing (spreadsheet/document)
  activeEntry?: OpenFileEntry | null;
  setOpenFiles?: React.Dispatch<React.SetStateAction<OpenFileEntry[]>>;
}

export function DesktopLayout(p: Props) {
  const uploadRef = useRef<HTMLInputElement>(null);
  const [inlineCreate, setInlineCreate] = useState<"file" | "dir" | null>(null);
  const finder = useExplorerFinder();
  const collapse = useCallback(() => p.setExplorerCollapsed(true), [p.setExplorerCollapsed]);
  const chrome = useExplorerChrome({
    // An open inline field or a live filter query means the reader is still
    // working in the panel, whatever the pointer is doing.
    holdOpen: inlineCreate !== null || finder.active,
    collapse,
  });

  const showExplorer = !p.explorerCollapsed;

  const handleUploadClick = () => uploadRef.current?.click();

  // Registered here rather than in the panel: these open the inline name field,
  // which is this layout's state, not an API call.
  useCommands({
    "explorer.newFile": () => setInlineCreate("file"),
    "explorer.newFolder": () => setInlineCreate("dir"),
    "explorer.upload": handleUploadClick,
    "layout.focusExplorer": () => p.setExplorerCollapsed(false),
  });
  const handleUploadChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      p.onUploadFiles?.(p.currentPath, e.target.files);
      e.target.value = "";
    }
  };

  const handleInlineSubmit = (name: string) => {
    if (inlineCreate === "file") p.onCreateFile?.(p.currentPath, name);
    else if (inlineCreate === "dir") p.onCreateDir?.(p.currentPath, name);
    setInlineCreate(null);
  };

  return (
    <div className="code-editor-panel code-editor-desktop" ref={p.editorRef} data-surface="editor">
      {/* Pinned it sits in the flow and the editor shrinks beside it;
          unpinned it overlays and withdraws on its own. */}
      <div
        className={`xpl${showExplorer ? " is-open" : ""}${chrome.pinned ? " is-pinned" : ""}`}
        style={{ width: chrome.width }}
        data-surface="explorer"
        aria-hidden={!showExplorer}
        onMouseEnter={chrome.cancelHide}
        onMouseLeave={chrome.scheduleHide}
      >
        <ExplorerHeader
          pinned={chrome.pinned}
          query={finder.query}
          searching={finder.searching}
          onQueryChange={finder.setQuery}
          onQueryClear={finder.clear}
          onTogglePinned={chrome.togglePinned}
          onCollapse={() => p.setExplorerCollapsed(true)}
          onCreateFile={p.onCreateFile ? () => setInlineCreate("file") : undefined}
          onCreateDir={p.onCreateDir ? () => setInlineCreate("dir") : undefined}
          onUploadFiles={p.onUploadFiles ? handleUploadClick : undefined}
          onReloadRoot={p.onReloadRoot}
        />
        <input
          ref={uploadRef}
          type="file"
          multiple
          style={{ display: "none" }}
          onChange={handleUploadChange}
        />

        {!finder.active && (
          <ExplorerOpenFiles
            openFiles={p.openFiles}
            activeFilePath={p.activeFilePath}
            onSelect={(f) => {
              p.setActiveFilePath(f.path);
              p.setSaveStatus(f.editedContent !== null ? "modified" : null);
            }}
            onClose={p.closeFile}
          />
        )}

        {/* Root inline create */}
        {inlineCreate && !finder.active && (
          <InlineNameField
            initial=""
            depth={0}
            placeholder={inlineCreate === "file" ? "filename.ext" : "folder name"}
            icon={inlineCreate === "dir"
              ? <FolderPlus size={12} className="xpl-field-icon" />
              : <FilePlus size={12} className="xpl-field-icon" />}
            onSubmit={handleInlineSubmit}
            onCancel={() => setInlineCreate(null)}
          />
        )}

        <div className="xpl-body">
          {finder.active ? (
            <ExplorerResults
              query={finder.query.trim()}
              results={finder.results}
              searching={finder.searching}
              error={finder.error}
              activeFilePath={p.activeFilePath}
              onOpenFile={(path) => { p.loadFile(path); finder.clear(); }}
              onOpenDir={(path) => { finder.clear(); p.toggleDir(path); }}
            />
          ) : p.loadingDir ? (
            <div className="xpl-state"><Loader2 size={14} className="spin" /> Loading files…</div>
          ) : p.entries.length === 0 ? (
            <div className="xpl-state">
              This folder is empty.
              <span className="xpl-state-hint">Use the menu above to add a file or folder.</span>
            </div>
          ) : (
            <ExplorerTree
              entries={p.entries}
              expandedDirs={p.expandedDirs}
              dirChildren={p.dirChildren}
              loadingDirs={p.loadingDirs}
              activeFilePath={p.activeFilePath}
              toggleDir={p.toggleDir}
              onFileClick={(path) => p.loadFile(path)}
              onCreateFile={p.onCreateFile}
              onCreateDir={p.onCreateDir}
              onDeleteFile={p.onDeleteFile}
              onDeleteDir={p.onDeleteDir}
              onReloadDir={p.onReloadDir}
              onReloadFile={p.onReloadFile}
              onRename={p.onRename}
              onDownloadFile={p.onDownloadFile}
              onDownloadDir={p.onDownloadDir}
            />
          )}
        </div>
        {/* Resize handle */}
        <div
          className="xpl-resize"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize explorer"
          onPointerDown={chrome.onResizeStart}
        />
      </div>

      {/* Editor area */}
      {/* The explorer is an overlay, so while it is open its own collapse
          control is the one on screen; showing a second, mirrored control
          underneath it only put a button on top of the filename. */}
      <div className={`code-editor-main${showExplorer ? "" : " has-expand"}`}>
        {!showExplorer && (
          <button
            type="button"
            className="xpl-expand"
            onClick={() => p.setExplorerCollapsed(false)}
            title="Show explorer"
            aria-label="Show explorer"
          >
            <PanelLeftOpen size={14} />
          </button>
        )}
        {p.openFile && (
          <EditorToolbar
            openFile={p.openFile}
            fileRenderType={p.fileRenderType}
            isModified={p.isModified}
            isDesktop
            activeView={p.activeView}
            setActiveView={p.setActiveView}
            lspAvailable={p.lspAvailable}
            lspBusy={p.lspBusy}
            activeDiagnostics={p.activeDiagnostics}
            handleHover={p.handleHover}
            handleDefinition={p.handleDefinition}
            handleFormatWithLsp={p.handleFormatWithLsp}
            saveStatus={p.saveStatus}
            saving={p.saving}
            handleSave={p.handleSave}
            handleRevert={p.handleRevert}
          />
        )}
        <EditorBody
          openFile={p.openFile}
          fileRenderType={p.fileRenderType}
          currentContent={p.currentContent}
          activeView={p.activeView}
          extensions={p.extensions}
          onEditorChange={p.onEditorChange}
          onCreateEditor={p.onCreateEditor}
          onUpdate={p.onUpdate}
          loadingFile={p.loadingFile}
          languageLoading={p.languageLoading}
          activeDiagnostics={p.activeDiagnostics}
          hoverText={p.hoverText}
          lspAvailable={p.lspAvailable}
          onJumpToLine={p.onJumpToLine}
          activeEntry={p.activeEntry}
          setOpenFiles={p.setOpenFiles}
        />
      </div>
    </div>
  );
}
