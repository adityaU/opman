import {
  ChevronLeft, Save, RotateCcw, Loader2,
  Code2, Eye, AlertCircle, Wand2, Info, ArrowRightCircle,
} from "lucide-react";
import type { FileReadResponse, FileRenderType, EditorLspDiagnostic, EditorViewMode } from "../types";
import { isPreviewableRenderType, isBinaryRenderType } from "../types";

interface Props {
  openFile: FileReadResponse;
  fileRenderType: FileRenderType;
  isModified: boolean;
  isDesktop: boolean;
  activeView: EditorViewMode;
  setActiveView: (mode: EditorViewMode) => void;
  // LSP
  lspAvailable: boolean;
  lspBusy: null | "hover" | "definition" | "format";
  activeDiagnostics: EditorLspDiagnostic[];
  handleHover: () => void;
  handleDefinition: () => void;
  handleFormatWithLsp: () => void;
  // Save
  saveStatus: "saved" | "modified" | null;
  saving: boolean;
  handleSave: () => void;
  handleRevert: () => void;
  // Mobile
  onBackToBrowser?: () => void;
}

export function EditorToolbar({
  openFile, fileRenderType, isModified, isDesktop,
  activeView, setActiveView,
  lspAvailable, lspBusy, activeDiagnostics,
  handleHover, handleDefinition, handleFormatWithLsp,
  saveStatus, saving, handleSave, handleRevert,
  onBackToBrowser,
}: Props) {
  return (
    <div className="code-editor-toolbar">
      {!isDesktop && onBackToBrowser && (
        <button className="code-editor-back" onClick={onBackToBrowser} title="Back to files" aria-label="Back to files">
          <ChevronLeft size={14} />
        </button>
      )}
      <span className="code-editor-filename" title={openFile.path}>{openFile.path}</span>
      {isModified && <span className="code-editor-modified-dot" title="Unsaved changes">&bull;</span>}
      <span className="code-editor-spacer" />

      {isPreviewableRenderType(fileRenderType) && (
        <div className="code-editor-view-tabs">
          <button className={`code-editor-view-tab ${activeView === "code" ? "active" : ""}`} onClick={() => setActiveView("code")}>
            <Code2 size={13} /> Code
          </button>
          <button className={`code-editor-view-tab ${activeView === "rendered" ? "active" : ""}`} onClick={() => setActiveView("rendered")}>
            <Eye size={13} /> Rendered
          </button>
        </div>
      )}

      {!isBinaryRenderType(fileRenderType) && (
        <div className="code-editor-lsp-group">
          <span className={`code-editor-lsp-pill ${lspAvailable ? "active" : "inactive"}`}>
            <AlertCircle size={12} /> {activeDiagnostics.length} issues
          </span>
          <button className="code-editor-action" onClick={handleHover} title="Hover info at cursor">
            {lspBusy === "hover" ? <Loader2 size={13} className="spin" /> : <Info size={13} />}
          </button>
          <button className="code-editor-action" onClick={handleDefinition} title="Go to definition">
            {lspBusy === "definition" ? <Loader2 size={13} className="spin" /> : <ArrowRightCircle size={13} />}
          </button>
          <button className="code-editor-action" onClick={handleFormatWithLsp} title="Format with LSP">
            {lspBusy === "format" ? <Loader2 size={13} className="spin" /> : <Wand2 size={13} />}
          </button>
        </div>
      )}

      {saveStatus === "saved" && <span className="code-editor-save-status">Saved</span>}
      {isModified && (
        <>
          <button className="code-editor-action" onClick={handleRevert} title="Revert changes" aria-label="Revert changes">
            <RotateCcw size={13} />
          </button>
          <button className="code-editor-action code-editor-save" onClick={handleSave} disabled={saving} title="Save (Cmd+S)" aria-label="Save file">
            {saving ? <Loader2 size={13} className="spin" /> : <Save size={13} />}
          </button>
        </>
      )}
    </div>
  );
}
