import { apiFetch, apiPost, getToken } from "./client";

// ── Types ─────────────────────────────────────────────

export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
}

export interface FileBrowseResponse {
  path: string;
  entries: FileEntry[];
}

export interface FileReadResponse {
  path: string;
  content: string;
  language: string;
}

export type FileRenderType =
  | "code" | "image" | "audio" | "video" | "markdown"
  | "html" | "mermaid" | "svg" | "csv" | "pdf" | "binary";

export interface EditorLspDiagnostic {
  file: string;
  lnum: number;
  col: number;
  severity: string;
  message: string;
  source: string;
}

export interface EditorDefinitionLocation {
  file: string;
  lnum: number;
  col: number;
}

// ── File browse / read / write ────────────────────────

export async function browseFiles(path?: string): Promise<FileBrowseResponse> {
  const qs = path ? `?path=${encodeURIComponent(path)}` : "";
  return apiFetch<FileBrowseResponse>(`/files${qs}`);
}

export async function readFile(path: string): Promise<FileReadResponse> {
  return apiFetch<FileReadResponse>(
    `/file/read?path=${encodeURIComponent(path)}`
  );
}

export async function writeFile(path: string, content: string): Promise<void> {
  return apiPost("/file/write", { path, content });
}

export function rawFileUrl(path: string): string {
  const token = getToken();
  const qs = `path=${encodeURIComponent(path)}${token ? `&token=${encodeURIComponent(token)}` : ""}`;
  return `/api/file/raw?${qs}`;
}

// ── File classification ───────────────────────────────

export function classifyFile(path: string): FileRenderType {
  const ext = path.split(".").pop()?.toLowerCase() || "";
  if (["png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "bmp", "avif"].includes(ext))
    return "image";
  if (["mp3", "wav", "ogg", "flac", "aac", "m4a", "weba"].includes(ext))
    return "audio";
  if (["mp4", "webm", "ogv", "mov", "avi", "mkv"].includes(ext))
    return "video";
  if (ext === "pdf") return "pdf";
  if (ext === "csv") return "csv";
  if (["md", "mdx", "markdown"].includes(ext)) return "markdown";
  if (["html", "htm"].includes(ext)) return "html";
  if (["mmd", "mermaid"].includes(ext)) return "mermaid";
  if (ext === "svg") return "svg";
  if (["xlsx", "xls", "pptx", "ppt", "docx", "doc", "zip", "tar", "gz", "rar", "7z", "exe", "dll", "so", "dylib", "wasm", "bin"].includes(ext))
    return "binary";
  return "code";
}

// ── LSP integration ───────────────────────────────────

export async function fetchEditorDiagnostics(
  path: string,
  sessionId: string
): Promise<{ diagnostics: EditorLspDiagnostic[]; available: boolean }> {
  return apiFetch(`/editor/lsp/diagnostics?path=${encodeURIComponent(path)}&session_id=${encodeURIComponent(sessionId)}`);
}

export async function fetchEditorHover(
  path: string,
  sessionId: string,
  line: number,
  col: number
): Promise<{ hover: string | null; available: boolean }> {
  return apiFetch(`/editor/lsp/hover?path=${encodeURIComponent(path)}&session_id=${encodeURIComponent(sessionId)}&line=${line}&col=${col}`);
}

export async function fetchEditorDefinition(
  path: string,
  sessionId: string,
  line: number,
  col: number
): Promise<{ locations: EditorDefinitionLocation[]; available: boolean }> {
  return apiFetch(`/editor/lsp/definition?path=${encodeURIComponent(path)}&session_id=${encodeURIComponent(sessionId)}&line=${line}&col=${col}`);
}

export async function formatEditorFile(
  path: string,
  sessionId: string
): Promise<{ formatted: boolean; content: string; available: boolean }> {
  return apiPost("/editor/lsp/format", { path, session_id: sessionId });
}
