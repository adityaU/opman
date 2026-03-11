/**
 * EditorBody — the main editor content area.
 *
 * Renders the CodeMirror editor, diagnostics panel, hover card,
 * and delegates to FileRenderers for non-code file types.
 */
import { Loader2, File } from "lucide-react";
import CodeMirror from "@uiw/react-codemirror";
import type { FileReadResponse, FileRenderType, EditorLspDiagnostic, EditorViewMode } from "../types";
import { isPreviewableRenderType } from "../types";
import {
  CsvViewer, MarkdownViewer, HtmlViewer, SvgViewer, MermaidViewer,
  ImagePreview, AudioPreview, VideoPreview, PdfPreview, BinaryPreview,
} from "./FileRenderers";

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
}

export function EditorBody({
  openFile, fileRenderType, currentContent, activeView,
  extensions, onEditorChange, onCreateEditor, onUpdate,
  loadingFile, languageLoading,
  activeDiagnostics, hoverText,
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
      />
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
}

function FileContent({
  openFile, fileRenderType, currentContent, activeView,
  extensions, onEditorChange, onCreateEditor, onUpdate,
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
    case "image":    return <ImagePreview file={openFile} />;
    case "audio":    return <AudioPreview file={openFile} />;
    case "video":    return <VideoPreview file={openFile} />;
    case "pdf":      return <PdfPreview file={openFile} />;
    case "csv":      return <CsvViewer content={openFile.content} />;
    case "markdown": return <MarkdownViewer content={currentContent} />;
    case "html":     return <HtmlViewer content={currentContent} />;
    case "mermaid":  return <MermaidViewer content={currentContent} />;
    case "svg":      return <SvgViewer content={currentContent} />;
    case "binary":   return <BinaryPreview file={openFile} />;
    case "code":
    default:
      return renderEditor();
  }
}
