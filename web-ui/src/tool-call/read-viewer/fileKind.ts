/** What a file path says about how it can be shown. Decided from the extension only. */
export enum FileKind {
  Image = "image",
  Svg = "svg",
  Video = "video",
  Audio = "audio",
  Pdf = "pdf",
  Stl = "stl",
  /** Anything else: the Read card keeps its text view. */
  Other = "other",
}

const BY_EXTENSION: Readonly<Record<string, FileKind>> = {
  png: FileKind.Image, jpg: FileKind.Image, jpeg: FileKind.Image, gif: FileKind.Image,
  webp: FileKind.Image, bmp: FileKind.Image, ico: FileKind.Image, avif: FileKind.Image,
  svg: FileKind.Svg,
  mp4: FileKind.Video, webm: FileKind.Video, mov: FileKind.Video, m4v: FileKind.Video,
  ogv: FileKind.Video,
  mp3: FileKind.Audio, wav: FileKind.Audio, ogg: FileKind.Audio, m4a: FileKind.Audio,
  flac: FileKind.Audio, aac: FileKind.Audio, weba: FileKind.Audio,
  pdf: FileKind.Pdf,
  stl: FileKind.Stl,
};

/** The lower-case extension of the last path segment, without the dot. */
export function extensionOf(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot <= 0 ? "" : name.slice(dot + 1).toLowerCase();
}

export function fileKindOf(path: string): FileKind {
  return BY_EXTENSION[extensionOf(path)] ?? FileKind.Other;
}

/** True when the kind has a real viewer (not the text fallback). */
export function isRenderable(kind: FileKind): boolean {
  return kind !== FileKind.Other;
}

export function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}
