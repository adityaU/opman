import { FileKind, fileKindOf } from "./fileKind";
import { sourceFor, type FileSource } from "./source";

export interface ViewerChoice {
  readonly kind: FileKind;
  readonly source: FileSource;
}

/**
 * Whether a Read call should show a real viewer, and from where.
 * An inline image in the output wins; otherwise the extension decides.
 * Returns null for anything else, so the card keeps its text view.
 */
export function chooseViewer(path: string, output: string): ViewerChoice | null {
  const source = sourceFor(path, output);
  if (!source) return null;
  if (source.kind === "inline") {
    const mime = source.url.slice(5, source.url.indexOf(";"));
    const kind = mime.startsWith("video/") ? FileKind.Video
      : mime.startsWith("audio/") ? FileKind.Audio
      : mime === "application/pdf" ? FileKind.Pdf
      : mime === "model/stl" ? FileKind.Stl : FileKind.Image;
    return { kind, source };
  }
  const kind = fileKindOf(path);
  return kind === FileKind.Other ? null : { kind, source };
}
