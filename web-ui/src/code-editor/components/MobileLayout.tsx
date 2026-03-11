/**
 * MobileLayout — full-screen file browser or editor for mobile breakpoints.
 */
import { Loader2, Folder, File } from "lucide-react";
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
}

export function MobileLayout(p: Props) {
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
      </div>
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
            <button
              key={entry.path}
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
          ))
        )}
      </div>
    </div>
  );
}
