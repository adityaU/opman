/**
 * EditorBody — the main editor content area.
 *
 * Renders the CodeMirror editor, diagnostics panel, hover card,
 * and delegates to FileRenderers for non-code file types.
 */
import { useState, useRef } from "react";
import { Loader2, File, PenLine } from "lucide-react";
import CodeMirror from "@uiw/react-codemirror";
import type { FileReadResponse, FileRenderType, EditorLspDiagnostic, EditorViewMode, OpenFileEntry } from "../types";
import { isPreviewableRenderType } from "../types";
import { rawFileUrl } from "../../api";
import {
  CsvViewer, MarkdownViewer, HtmlViewer, SvgViewer, MermaidViewer,
  ImagePreview, AudioPreview, VideoPreview, BinaryPreview,
} from "./FileRenderers";
import { PdfCanvasViewer } from "./PdfCanvasViewer";
import { SpreadsheetEditor } from "./SpreadsheetEditor";
import { DocumentEditor } from "./DocumentEditor";
import { ImagePreviewZoom } from "./ImagePreviewZoom";
import { CadViewer } from "./CadViewer";
import { MarkupOverlay } from "./MarkupOverlay";

interface Props {
  openFile: FileReadResponse | null;
  fileRenderType: FileRenderType;
  currentContent: string;
  activeView: EditorViewMode;
  // Editor
  extensions: any[];
  onEditorChange: (value: string) => void;
  onCreateEditor: (view: any) => void;
  onUpdate: (update: any) => void;
  // State flags
  loadingFile: boolean;
  languageLoading: boolean;
  // Diagnostics / hover
  activeDiagnostics: EditorLspDiagnostic[];
  hoverText: string | null;
  // Doc-type editing (spreadsheet/document)
  activeEntry?: OpenFileEntry | null;
  setOpenFiles?: React.Dispatch<React.SetStateAction<OpenFileEntry[]>>;
}

export function EditorBody({
  openFile, fileRenderType, currentContent, activeView,
  extensions, onEditorChange, onCreateEditor, onUpdate,
  loadingFile, languageLoading,
  activeDiagnostics, hoverText,
  activeEntry, setOpenFiles,
}: Props) {
  if (loadingFile) {
    return (
      <div className="code-editor-body">
        <div className="code-editor-loading">
          <Loader2 size={20} className="spin" />
          <span>Loading...</span>
        </div>
      </div>
    );
  }

  if (!openFile) {
    return (
      <div className="code-editor-body">
        <div className="code-editor-empty-state">
          <File size={32} strokeWidth={1} />
          <span>Select a file to edit</span>
        </div>
      </div>
    );
  }

  return (
    <div className="code-editor-body">
      {hoverText && (
        <div className="code-editor-hover-card">
          <div className="code-editor-hover-title">Hover</div>
          <pre>{hoverText}</pre>
        </div>
      )}
      {activeDiagnostics.length > 0 && (
        <div className="code-editor-diagnostics">
          {activeDiagnostics.slice(0, 6).map((diag, idx) => (
            <div
              key={`${diag.lnum}-${diag.col}-${idx}`}
              className={`code-editor-diagnostic severity-${diag.severity.toLowerCase()}`}
            >
              <span className="code-editor-diagnostic-pos">L{diag.lnum}:C{diag.col}</span>
              <span className="code-editor-diagnostic-msg">{diag.message}</span>
            </div>
          ))}
        </div>
      )}
      {languageLoading && (
        <div className="code-editor-loading-inline">
          <Loader2 size={16} className="spin" />
          <span>Loading language tools...</span>
        </div>
      )}
      <FileContent
        openFile={openFile}
        fileRenderType={fileRenderType}
        currentContent={currentContent}
        activeView={activeView}
        extensions={extensions}
        onEditorChange={onEditorChange}
        onCreateEditor={onCreateEditor}
        onUpdate={onUpdate}
        activeEntry={activeEntry}
        setOpenFiles={setOpenFiles}
      />
    </div>
  );
}

// ── Markup wrapper ──────────────────────────────────────

function MarkupWrapper({ children }: { children: React.ReactNode }) {
  const [markupActive, setMarkupActive] = useState(false);
  const previewRef = useRef<HTMLDivElement>(null);

  return (
    <div className="markup-preview-wrapper" ref={previewRef}>
      {children}
      {!markupActive && (
        <button className="markup-start-btn" onClick={() => setMarkupActive(true)}>
          <PenLine size={13} /> Markup
        </button>
      )}
      {markupActive && (
        <MarkupOverlay previewRef={previewRef} onClose={() => setMarkupActive(false)} />
      )}
    </div>
  );
}

// ── File content dispatcher ─────────────────────────────

interface FileContentProps {
  openFile: FileReadResponse;
  fileRenderType: FileRenderType;
  currentContent: string;
  activeView: EditorViewMode;
  extensions: any[];
  onEditorChange: (value: string) => void;
  onCreateEditor: (view: any) => void;
  onUpdate: (update: any) => void;
  activeEntry?: OpenFileEntry | null;
  setOpenFiles?: React.Dispatch<React.SetStateAction<OpenFileEntry[]>>;
}

function FileContent({
  openFile, fileRenderType, currentContent, activeView,
  extensions, onEditorChange, onCreateEditor, onUpdate,
  activeEntry, setOpenFiles,
}: FileContentProps) {
  const renderEditor = () => (
    <CodeMirror
      value={currentContent}
      onChange={onEditorChange}
      onCreateEditor={onCreateEditor}
      onUpdate={onUpdate}
      extensions={extensions}
      theme="none"
      basicSetup={{
        lineNumbers: true,
        highlightActiveLineGutter: true,
        highlightActiveLine: true,
        foldGutter: true,
        bracketMatching: true,
        closeBrackets: true,
        autocompletion: true,
        indentOnInput: true,
      }}
    />
  );

  // Previewable types in code mode show the editor
  if (isPreviewableRenderType(fileRenderType) && activeView === "code") {
    return renderEditor();
  }

  switch (fileRenderType) {
    case "spreadsheet":
      if (activeEntry?.docData && setOpenFiles) {
        return <SpreadsheetEditor path={openFile.path} docData={activeEntry.docData} setOpenFiles={setOpenFiles} />;
      }
      return <BinaryPreview file={openFile} />;
    case "document":
      if (activeEntry?.docData && setOpenFiles) {
        return <DocumentEditor path={openFile.path} docData={activeEntry.docData} setOpenFiles={setOpenFiles} />;
      }
      return <BinaryPreview file={openFile} />;
    case "model3d":  return <MarkupWrapper><CadViewer path={openFile.path} /></MarkupWrapper>;
    case "image":    return <MarkupWrapper><ImagePreviewZoom url={rawFileUrl(openFile.path)} alt={openFile.path} /></MarkupWrapper>;
    case "audio":    return <AudioPreview file={openFile} />;
    case "video":    return <MarkupWrapper><VideoPreview file={openFile} /></MarkupWrapper>;
    case "pdf":      return <MarkupWrapper><PdfCanvasViewer url={rawFileUrl(openFile.path)} alt={openFile.path} /></MarkupWrapper>;
    case "csv":      return <CsvViewer content={openFile.content} />;
    case "markdown": return <MarkdownViewer content={currentContent} />;
    case "html":     return <HtmlViewer content={currentContent} />;
    case "mermaid":  return <MarkupWrapper><MermaidViewer content={currentContent} /></MarkupWrapper>;
    case "svg":      return <MarkupWrapper><SvgViewer content={currentContent} /></MarkupWrapper>;
    case "binary":   return <BinaryPreview file={openFile} />;
    case "code":
    default:
      return renderEditor();
  }
}
