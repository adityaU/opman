import React, { useCallback, useEffect, useState } from "react";
import { Download, ExternalLink } from "lucide-react";
import { useOpenFileInEditor } from "../EditorOpenContext";
import { FileKind, baseName } from "./fileKind";
import { downloadHref, type FileSource } from "./source";
import { ImageViewer } from "./ImageViewer";
import { MediaViewer } from "./MediaViewer";
import { PdfViewer } from "./PdfViewer";
import { StlViewer } from "./StlViewer";

interface ReadViewerProps {
  readonly kind: FileKind;
  readonly path: string;
  readonly source: FileSource;
}

/** The expanded body of a Read card for a file the browser can show. Mounted only when open. */
export function ReadViewer({ kind, path, source }: ReadViewerProps) {
  const openFile = useOpenFileInEditor();
  const [meta, setMeta] = useState("");
  const name = baseName(path || "image");
  const href = downloadHref(source);
  const onMeta = useCallback((text: string) => setMeta(text), []);
  useEffect(() => setMeta(""), [source.url]);

  return (
    <div className={`rv rv-kind-${kind}`}>
      <div className="rv-bar">
        <span className="rv-name" title={path}>{name}</span>
        {meta && <span className="rv-meta">{meta}</span>}
        <span className="rv-actions">
          {openFile && source.kind === "server" && (
            <button type="button" className="rv-action" onClick={() => openFile(path)} title="Open in pane">
              <ExternalLink size={12} aria-hidden="true" /><span>Open</span>
            </button>
          )}
          {href && (
            <a className="rv-action" href={href} download={name} title="Download">
              <Download size={12} aria-hidden="true" /><span>Save</span>
            </a>
          )}
        </span>
      </div>
      <div className="rv-body">{renderViewer(kind, name, source, href, onMeta)}</div>
    </div>
  );
}

function renderViewer(
  kind: FileKind, name: string, source: FileSource, href: string | null,
  onMeta: (text: string) => void,
): React.ReactNode {
  switch (kind) {
    case FileKind.Image:
    case FileKind.Svg:
      return <ImageViewer url={source.url} name={name} onMeta={onMeta} />;
    case FileKind.Video:
      return <MediaViewer url={source.url} video onMeta={onMeta} />;
    case FileKind.Audio:
      return <MediaViewer url={source.url} video={false} onMeta={onMeta} />;
    case FileKind.Pdf:
      return <PdfViewer url={source.url} name={name} href={href} onMeta={onMeta} />;
    case FileKind.Stl:
      return <StlViewer url={source.url} href={href} onMeta={onMeta} />;
    case FileKind.Other:
      return null;
  }
}
