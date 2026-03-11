import { useState, useEffect, useMemo } from "react";
import DOMPurify from "dompurify";
import mermaid from "mermaid";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { File } from "lucide-react";
import { rawFileUrl, type FileReadResponse } from "../../api";

// ── CSV ─────────────────────────────────────────────────

export function CsvViewer({ content }: { content: string }) {
  const rows = useMemo(() => {
    if (!content.trim()) return [];
    return content.split("\n").map((line) => {
      const cells: string[] = [];
      let current = "";
      let inQuotes = false;
      for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (ch === '"') {
          if (inQuotes && line[i + 1] === '"') { current += '"'; i++; }
          else inQuotes = !inQuotes;
        } else if (ch === "," && !inQuotes) {
          cells.push(current.trim()); current = "";
        } else {
          current += ch;
        }
      }
      cells.push(current.trim());
      return cells;
    });
  }, [content]);

  if (rows.length === 0) {
    return <div className="file-preview file-preview-binary"><span>Empty CSV file</span></div>;
  }

  const header = rows[0];
  const body = rows.slice(1).filter((r) => r.some((c) => c.length > 0));

  return (
    <div className="csv-viewer">
      <table>
        <thead><tr>{header.map((cell, i) => <th key={i}>{cell}</th>)}</tr></thead>
        <tbody>{body.map((row, ri) => <tr key={ri}>{row.map((cell, ci) => <td key={ci}>{cell}</td>)}</tr>)}</tbody>
      </table>
    </div>
  );
}

// ── Markdown ────────────────────────────────────────────

export function MarkdownViewer({ content }: { content: string }) {
  return (
    <div className="markdown-viewer">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
    </div>
  );
}

// ── HTML ────────────────────────────────────────────────

export function HtmlViewer({ content }: { content: string }) {
  const sanitized = useMemo(() => DOMPurify.sanitize(content), [content]);
  return <iframe className="html-viewer-frame" sandbox="allow-scripts allow-same-origin" srcDoc={sanitized} title="HTML preview" />;
}

// ── SVG ─────────────────────────────────────────────────

export function SvgViewer({ content }: { content: string }) {
  const sanitized = useMemo(
    () => DOMPurify.sanitize(content, { USE_PROFILES: { svg: true, svgFilters: true } }),
    [content],
  );
  return <div className="svg-viewer" dangerouslySetInnerHTML={{ __html: sanitized }} />;
}

// ── Mermaid ─────────────────────────────────────────────

export function MermaidViewer({ content }: { content: string }) {
  const [svg, setSvg]     = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const id = `mermaid-${Math.random().toString(36).slice(2, 10)}`;
    mermaid.initialize({ startOnLoad: false, theme: "base", securityLevel: "strict" });
    mermaid.render(id, content)
      .then((result) => { if (active) { setSvg(result.svg); setError(null); } })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err.message : "Failed to render Mermaid diagram");
          setSvg("");
        }
      });
    return () => { active = false; };
  }, [content]);

  if (error) return <div className="file-preview file-preview-binary">{error}</div>;
  return <div className="mermaid-viewer" dangerouslySetInnerHTML={{ __html: svg }} />;
}

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
