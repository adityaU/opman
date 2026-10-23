import { File } from "lucide-react";
import { rawFileUrl, type FileReadResponse } from "../../api";

export { CsvViewer, MarkdownViewer, SvgViewer } from "../../content-viewers/MarkupViewers";
export { HtmlViewer } from "../../content-viewers/HtmlViewer";
export { MermaidViewer } from "../../content-viewers/MermaidViewer";

// ── Media / Binary previews ─────────────────────────────

export function ImagePreview({ file }: { file: FileReadResponse }) {
  return <div className="file-preview file-preview-image"><img src={rawFileUrl(file.path)} alt={file.path} /></div>;
}

export function AudioPreview({ file }: { file: FileReadResponse }) {
  return (
    <div className="file-preview file-preview-audio">
      <div className="file-preview-icon"><File size={48} strokeWidth={1} /></div>
      <span className="file-preview-name">{file.path.split("/").pop()}</span>
      <audio controls src={rawFileUrl(file.path)} preload="metadata">Your browser does not support the audio element.</audio>
    </div>
  );
}

export function VideoPreview({ file }: { file: FileReadResponse }) {
  return (
    <div className="file-preview file-preview-video">
      <video controls src={rawFileUrl(file.path)} preload="metadata">Your browser does not support the video element.</video>
    </div>
  );
}

export function PdfPreview({ file }: { file: FileReadResponse }) {
  return <div className="file-preview file-preview-pdf"><iframe src={rawFileUrl(file.path)} title={file.path} /></div>;
}

export function BinaryPreview({ file }: { file: FileReadResponse }) {
  return (
    <div className="file-preview file-preview-binary">
      <File size={48} strokeWidth={1} />
      <span className="file-preview-label">Binary file — cannot be displayed</span>
      <span className="file-preview-name">{file.path.split("/").pop()}</span>
    </div>
  );
}
