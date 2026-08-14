import { apiFetch, apiPost, apiUpload } from "./client";
import { editorSocket, type EditorOp } from "./editorSocket";

/**
 * Every editor query goes over the binary channel rather than its own POST.
 * The signatures are unchanged, so no call site knows the difference — see
 * `editorSocket` for why the transport matters under a fast pointer.
 */
function lspRequest<T>(op: EditorOp, payload: unknown, signal?: AbortSignal): Promise<T> {
  return editorSocket.request(op, payload, signal) as Promise<T>;
}

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

export interface FileUploadResponse {
  files: string[];
}

// ── Document preview types ────────────────────────────

export interface DocReadResponse {
  path: string;
  data: DocData;
}

export type DocData =
  | { type: "spreadsheet"; sheets: SheetData[] }
  | { type: "document"; html: string }
  | { type: "presentation"; slides: SlideData[] };

export interface SheetData {
  name: string;
  rows: string[][];
}

export interface SlideData {
  title: string;
  content: string;
}

export type FileRenderType =
  | "code" | "image" | "audio" | "video" | "markdown"
  | "html" | "mermaid" | "svg" | "csv" | "pdf"
  | "spreadsheet" | "document" | "model3d" | "binary";

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

// ── File search (fuzzy) ───────────────────────────────

export interface FileSearchEntry {
  name: string;
  path: string;
  is_dir: boolean;
}

export interface FileSearchResponse {
  query: string;
  entries: FileSearchEntry[];
}

export async function searchFiles(query: string, limit = 20): Promise<FileSearchEntry[]> {
  if (!query.trim()) return [];
  const qs = `?q=${encodeURIComponent(query)}&limit=${limit}`;
  const resp = await apiFetch<FileSearchResponse>(`/files/search${qs}`);
  return resp.entries;
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
  return `/api/file/raw?path=${encodeURIComponent(path)}`;
}

export function fileDownloadUrl(path: string): string {
  return `/api/file/download?path=${encodeURIComponent(path)}`;
}

export function dirDownloadUrl(path: string): string {
  return `/api/dir/download?path=${encodeURIComponent(path)}`;
}

// ── Document read (spreadsheet / docx) ────────────────

export async function docRead(path: string): Promise<DocReadResponse> {
  return apiFetch<DocReadResponse>(
    `/file/doc-read?path=${encodeURIComponent(path)}`
  );
}

// ── File / directory create / delete / upload ─────────

export async function createFile(path: string, content?: string): Promise<void> {
  return apiPost("/file/create", { path, content: content ?? "" });
}

export async function createDir(path: string): Promise<void> {
  return apiPost("/dir/create", { path });
}

export async function deleteFile(path: string): Promise<void> {
  return apiPost("/file/delete", { path });
}

export async function deleteDir(path: string): Promise<void> {
  return apiPost("/dir/delete", { path });
}

export async function renameEntry(fromPath: string, toPath: string): Promise<void> {
  return apiPost("/rename", { from_path: fromPath, to_path: toPath });
}

/** Trigger a browser download for a file or directory (zip). */
export function triggerDownload(url: string): void {
  const a = document.createElement("a");
  a.href = url;
  a.download = "";
  document.body.appendChild(a);
  a.click();
  a.remove();
}

export async function uploadFiles(
  directory: string,
  files: FileList | File[],
): Promise<FileUploadResponse> {
  const formData = new FormData();
  formData.append("directory", directory);
  for (const file of files) {
    formData.append("files", file);
  }
  return apiUpload<FileUploadResponse>("/file/upload", formData);
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
  if (["stl", "obj", "gltf", "glb", "ply", "3mf", "fbx", "dae"].includes(ext))
    return "model3d";
  if (["xlsx", "xls", "ods", "xlsb", "tsv"].includes(ext))
    return "spreadsheet";
  if (ext === "docx") return "document";
  if (["pptx", "ppt", "doc", "zip", "tar", "gz", "rar", "7z", "exe", "dll", "so", "dylib", "wasm", "bin"].includes(ext))
    return "binary";
  return "code";
}

// ── LSP integration ───────────────────────────────────
//
// These are POSTs rather than GETs because they carry the editor's current
// buffer. Without it the language server would answer about the file as last
// saved, so every position after the first unsaved edit would describe the
// wrong symbol — confidently, which is worse than not answering.

export async function fetchEditorDiagnostics(
  path: string,
  sessionId: string,
  content?: string
): Promise<{ diagnostics: EditorLspDiagnostic[]; available: boolean; published: boolean }> {
  return lspRequest("diagnostics", { path, session_id: sessionId, content });
}

/** The four navigations a hover card can offer, as the server names them. */
export type EditorGotoKind = "definition" | "type-definition" | "implementation" | "declaration";

/** What the language server behind this file will actually answer. */
export interface EditorLspActions {
  definition: boolean;
  typeDefinition: boolean;
  implementation: boolean;
  declaration: boolean;
  references: boolean;
  rename: boolean;
  format: boolean;
}

export interface EditorHoverResponse {
  hover: string | null;
  available: boolean;
  /** Absent from an older backend, which is why every reader defaults it. */
  actions?: EditorLspActions;
}

export async function fetchEditorHover(
  path: string,
  sessionId: string,
  line: number,
  col: number,
  content?: string,
  signal?: AbortSignal
): Promise<EditorHoverResponse> {
  return lspRequest("hover", { path, session_id: sessionId, line, col, content }, signal);
}

export async function fetchEditorDefinition(
  path: string,
  sessionId: string,
  line: number,
  col: number,
  content?: string,
  goto: EditorGotoKind = "definition"
): Promise<{ locations: EditorDefinitionLocation[]; available: boolean }> {
  return lspRequest("goto", { path, session_id: sessionId, line, col, content, goto });
}

export interface EditorReferenceLocation {
  file: string;
  lnum: number;
  col: number;
  text: string;
}

export async function fetchEditorReferences(
  path: string,
  sessionId: string,
  line: number,
  col: number,
  content?: string
): Promise<{ locations: EditorReferenceLocation[]; available: boolean }> {
  return lspRequest("references", { path, session_id: sessionId, line, col, content });
}

export async function renameEditorSymbol(
  path: string,
  sessionId: string,
  line: number,
  col: number,
  newName: string,
  content?: string
): Promise<{ renamed: boolean; files: string[]; available: boolean }> {
  return lspRequest("rename", {
    path, session_id: sessionId, line, col, content, new_name: newName,
  });
}

export interface EditorCompletionItem {
  label: string;
  kind: string;
  detail: string;
  documentation: string | null;
  insert: string;
  snippet: boolean;
  sort: string;
  filter: string;
  preselect: boolean;
  deprecated: boolean;
}

export interface EditorCompletionResponse {
  available: boolean;
  items: EditorCompletionItem[];
  incomplete: boolean;
  triggerCharacters: string[];
}

export async function fetchEditorCompletion(
  path: string,
  sessionId: string,
  line: number,
  col: number,
  content?: string,
  trigger?: string
): Promise<EditorCompletionResponse> {
  return lspRequest("completion", {
    path, session_id: sessionId, line, col, content, trigger,
  });
}

export async function formatEditorFile(
  path: string,
  sessionId: string,
  content?: string
): Promise<{ formatted: boolean; content: string; available: boolean }> {
  return lspRequest("format", { path, session_id: sessionId, content });
}
