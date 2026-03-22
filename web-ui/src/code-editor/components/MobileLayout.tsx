/**
 * MobileLayout — full-screen file browser or editor for mobile breakpoints.
 */
import { useRef, useState, useCallback, useEffect } from "react";
import { createPortal } from "react-dom";
import { Loader2, Folder, File, FilePlus, FolderPlus, Upload, Trash2, X, MoreVertical, RefreshCw } from "lucide-react";
import type {
  FileReadResponse, FileRenderType, EditorLspDiagnostic,
  EditorViewMode, BreadcrumbEntry, FileEntry,
} from "../types";
import { formatSize } from "../types";
import { EditorToolbar } from "./EditorToolbar";
import { EditorBody } from "./EditorBody";

interface Props {
  editorRef: React.RefObject<HTMLDivElement>;
  // Browser state
  breadcrumbs: BreadcrumbEntry[];
  entries: FileEntry[];
  loadingDir: boolean;
  loadDirectory: (path: string) => Promise<void>;
  currentPath: string;
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
  // Navigation
  onEntryClick: (entry: FileEntry) => void;
  onBackToBrowser: () => void;
  // File management
  onCreateFile?: (parentDir: string, name: string) => void;
  onCreateDir?: (parentDir: string, name: string) => void;
  onDeleteFile?: (path: string) => void;
  onDeleteDir?: (path: string) => void;
  onUploadFiles?: (dir: string, files: FileList | File[]) => void;
  onReloadRoot?: () => void;
  fileActionBusy?: boolean;
}

export function MobileLayout(p: Props) {
  const uploadRef = useRef<HTMLInputElement>(null);
  const toggleRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const [showActions, setShowActions] = useState(false);
  const [dropdownPos, setDropdownPos] = useState<{ top: number; left: number } | null>(null);
  const [inlineCreate, setInlineCreate] = useState<"file" | "dir" | null>(null);
  const [inlineValue, setInlineValue] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<{ path: string; isDir: boolean; name: string } | null>(null);

  const openDropdown = useCallback(() => {
    if (toggleRef.current) {
      const rect = toggleRef.current.getBoundingClientRect();
      const menuWidth = 150;
      const pad = 4;
      // Align right edge of menu to right edge of button
      let left = rect.right - menuWidth;
      // Clamp so menu never goes off left edge
      if (left < pad) left = pad;
      // Clamp so menu never goes off right edge
      if (left + menuWidth > window.innerWidth - pad) left = window.innerWidth - menuWidth - pad;
      setDropdownPos({ top: rect.bottom + 2, left });
    }
    setShowActions(true);
  }, []);

  // close on outside click
  useEffect(() => {
    if (!showActions) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node;
      if (toggleRef.current && toggleRef.current.contains(target)) return;
      if (dropdownRef.current && dropdownRef.current.contains(target)) return;
      setShowActions(false);
    };
    document.addEventListener("mousedown", handler, true);
    return () => document.removeEventListener("mousedown", handler, true);
  }, [showActions]);

  const handleUploadClick = () => { uploadRef.current?.click(); setShowActions(false); };
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

  // File is open — show editor
  if (p.openFile) {
    return (
      <div className="code-editor-panel" ref={p.editorRef}>
        <EditorToolbar
          openFile={p.openFile}
          fileRenderType={p.fileRenderType}
          isModified={p.isModified}
          isDesktop={false}
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
          onBackToBrowser={p.onBackToBrowser}
        />
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
    );
  }

  // No file open — show file browser
  const hasActions = p.onCreateFile || p.onCreateDir || p.onUploadFiles || p.onReloadRoot;
  return (
    <div className="code-editor-panel">
      <div className="code-editor-toolbar">
        <div className="code-editor-breadcrumbs">
          {p.breadcrumbs.map((crumb, i) => (
            <span key={crumb.path}>
              {i > 0 && <span className="breadcrumb-sep">/</span>}
              <button className="breadcrumb-link" onClick={() => p.loadDirectory(crumb.path)}>
                {crumb.label}
              </button>
            </span>
          ))}
        </div>
        {hasActions && (
          <div className="mobile-explorer-actions-toggle">
            <button ref={toggleRef} className="explorer-hdr-btn" onClick={() => showActions ? setShowActions(false) : openDropdown()} title="File actions">
              <MoreVertical size={14} />
            </button>
            {showActions && dropdownPos && createPortal(
              <div ref={dropdownRef} className="mobile-explorer-actions-dropdown" style={{ position: "fixed", top: dropdownPos.top, left: dropdownPos.left }}>
                {p.onCreateFile && (
                  <button className="mobile-action-item" onClick={() => { setInlineCreate("file"); setInlineValue(""); setShowActions(false); }}>
                    <FilePlus size={13} /> New File
                  </button>
                )}
                {p.onCreateDir && (
                  <button className="mobile-action-item" onClick={() => { setInlineCreate("dir"); setInlineValue(""); setShowActions(false); }}>
                    <FolderPlus size={13} /> New Folder
                  </button>
                )}
                {p.onUploadFiles && (
                  <button className="mobile-action-item" onClick={handleUploadClick}>
                    <Upload size={13} /> Upload
                  </button>
                )}
                {p.onReloadRoot && (
                  <button className="mobile-action-item" onClick={() => { p.onReloadRoot?.(); setShowActions(false); }}>
                    <RefreshCw size={13} /> Reload
                  </button>
                )}
              </div>,
              document.body
            )}
          </div>
        )}
        <input
          ref={uploadRef}
          type="file"
          multiple
          style={{ display: "none" }}
          onChange={handleUploadChange}
        />
      </div>

      {/* Inline create */}
      {inlineCreate && (
        <div className="mobile-inline-create">
          {inlineCreate === "dir"
            ? <FolderPlus size={14} className="file-icon folder-icon" />
            : <FilePlus size={14} className="file-icon" />}
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
      )}

      {/* Confirm delete overlay */}
      {confirmDelete && (
        <div className="mobile-confirm-delete">
          <span>Delete {confirmDelete.isDir ? "folder " : ""}<strong>{confirmDelete.name}</strong>?</span>
          <button className="explorer-confirm-yes" onClick={() => {
            if (confirmDelete.isDir) p.onDeleteDir?.(confirmDelete.path);
            else p.onDeleteFile?.(confirmDelete.path);
            setConfirmDelete(null);
          }}>Delete</button>
          <button className="explorer-confirm-no" onClick={() => setConfirmDelete(null)}><X size={12} /></button>
        </div>
      )}

      <div className="code-editor-filelist">
        {p.loadingDir ? (
          <div className="code-editor-loading">
            <Loader2 size={20} className="spin" />
            <span>Loading...</span>
          </div>
        ) : p.entries.length === 0 ? (
          <div className="code-editor-empty">Empty directory</div>
        ) : (
          p.entries.map((entry) => (
            <div key={entry.path} className="code-editor-file-entry-row">
              <button
                className="code-editor-file-entry"
                onClick={() => p.onEntryClick(entry)}
              >
                {entry.is_dir ? (
                  <Folder size={14} className="file-icon folder-icon" />
                ) : (
                  <File size={14} className="file-icon" />
                )}
                <span className="file-name">{entry.name}</span>
                {!entry.is_dir && <span className="file-size">{formatSize(entry.size)}</span>}
              </button>
              {(entry.is_dir ? p.onDeleteDir : p.onDeleteFile) && (
                <button
                  className="mobile-entry-delete-btn"
                  title={`Delete ${entry.name}`}
                  onClick={() => setConfirmDelete({ path: entry.path, isDir: entry.is_dir, name: entry.name })}
                >
                  <Trash2 size={12} />
                </button>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
