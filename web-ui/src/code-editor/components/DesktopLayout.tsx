/**
 * DesktopLayout — side-by-side file explorer + editor for desktop breakpoints.
 */
import { useRef, useState } from "react";
import { Loader2, File, X, PanelLeftClose, PanelLeftOpen, FilePlus, FolderPlus, Upload } from "lucide-react";
import type { OpenFileEntry, FileReadResponse, FileRenderType, EditorLspDiagnostic, EditorViewMode, FileEntry } from "../types";
import { EditorToolbar } from "./EditorToolbar";
import { EditorBody } from "./EditorBody";
import { ExplorerTree } from "./ExplorerTree";

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
  fileActionBusy?: boolean;
}

export function DesktopLayout(p: Props) {
  const uploadRef = useRef<HTMLInputElement>(null);
  const [inlineCreate, setInlineCreate] = useState<"file" | "dir" | null>(null);
  const [inlineValue, setInlineValue] = useState("");

  const handleUploadClick = () => uploadRef.current?.click();
  const handleUploadChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      p.onUploadFiles?.(p.currentPath, e.target.files);
      e.target.value = "";
    }
  };

  const handleInlineSubmit = () => {
    const trimmed = inlineValue.trim();
    if (trimmed) {
      if (inlineCreate === "file") p.onCreateFile?.(p.currentPath, trimmed);
      else if (inlineCreate === "dir") p.onCreateDir?.(p.currentPath, trimmed);
    }
    setInlineCreate(null);
    setInlineValue("");
  };

  return (
    <div className="code-editor-panel code-editor-desktop" ref={p.editorRef}>
      {/* File explorer (collapsible) */}
      {!p.explorerCollapsed && (
        <div className="code-editor-explorer">
          <div className="explorer-header">
            <span className="explorer-title">Explorer</span>
            <span className="explorer-header-actions">
              {p.onCreateFile && (
                <button className="explorer-hdr-btn" title="New file" onClick={() => { setInlineCreate("file"); setInlineValue(""); }}>
                  <FilePlus size={13} />
                </button>
              )}
              {p.onCreateDir && (
                <button className="explorer-hdr-btn" title="New folder" onClick={() => { setInlineCreate("dir"); setInlineValue(""); }}>
                  <FolderPlus size={13} />
                </button>
              )}
              {p.onUploadFiles && (
                <button className="explorer-hdr-btn" title="Upload files" onClick={handleUploadClick}>
                  <Upload size={13} />
                </button>
              )}
              <button
                className="explorer-collapse-btn"
                onClick={() => p.setExplorerCollapsed(true)}
                title="Collapse explorer"
                aria-label="Collapse explorer"
              >
                <PanelLeftClose size={14} />
              </button>
            </span>
            <input
              ref={uploadRef}
              type="file"
              multiple
              style={{ display: "none" }}
              onChange={handleUploadChange}
            />
          </div>

          {/* Open files list */}
          {p.openFiles.length > 0 && (
            <div className="explorer-open-files">
              <div className="explorer-section-label">Open Files</div>
              {p.openFiles.map((f) => {
                const name = f.path.split("/").pop() || f.path;
                const isActive = f.path === p.activeFilePath;
                return (
                  <div
                    key={f.path}
                    className={`explorer-open-file${isActive ? " active" : ""}`}
                    onClick={() => {
                      p.setActiveFilePath(f.path);
                      p.setSaveStatus(f.editedContent !== null ? "modified" : null);
                    }}
                    title={f.path}
                  >
                    <File size={13} className="file-icon" />
                    <span className="file-name">{name}</span>
                    {f.editedContent !== null && <span className="open-file-modified-dot" />}
                    <button
                      className="open-file-close"
                      onClick={(e) => { e.stopPropagation(); p.closeFile(f.path); }}
                      aria-label={`Close ${name}`}
                    >
                      <X size={12} />
                    </button>
                  </div>
                );
              })}
            </div>
          )}

          {/* Root inline create (from header buttons) */}
          {inlineCreate && (
            <div className="explorer-root-inline-create">
              <div className="explorer-inline-input" style={{ paddingLeft: "8px" }}>
                {inlineCreate === "dir"
                  ? <FolderPlus size={13} className="file-icon folder-icon" />
                  : <FilePlus size={13} className="file-icon" />}
                <input
                  className="explorer-inline-name-input"
                  value={inlineValue}
                  placeholder={inlineCreate === "file" ? "filename" : "folder name"}
                  autoFocus
                  onChange={(e) => setInlineValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleInlineSubmit();
                    if (e.key === "Escape") { setInlineCreate(null); setInlineValue(""); }
                  }}
                  onBlur={handleInlineSubmit}
                />
              </div>
            </div>
          )}

          <div className="explorer-section-label">Files</div>
          <div className="explorer-tree">
            {p.loadingDir ? (
              <div className="code-editor-loading"><Loader2 size={16} className="spin" /></div>
            ) : p.entries.length === 0 ? (
              <div className="code-editor-empty">Empty directory</div>
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
              />
            )}
          </div>
        </div>
      )}

      {/* Editor area */}
      <div className="code-editor-main">
        {p.explorerCollapsed && (
          <button
            className="explorer-expand-btn"
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
        />
      </div>
    </div>
  );
}
