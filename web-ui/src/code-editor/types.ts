/**
 * Code editor domain types.
 */
import type { FileReadResponse, FileRenderType, FileEntry, EditorLspDiagnostic, DocData, SheetData } from "../api";

// ── View mode ───────────────────────────────────────────

export type EditorViewMode = "code" | "rendered";

// ── Open file tracking ──────────────────────────────────

export interface OpenFileEntry {
  path: string;
  content: string;
  language: string;
  renderType: FileRenderType;
  /** Non-null when the file has unsaved edits */
  editedContent: string | null;
  /** Structured document data (spreadsheet sheets, document HTML) from doc-read. */
  docData: DocData | null;
  /** Edited document data — set when user modifies a spreadsheet/document in the UI. */
  editedDocData: DocData | null;
}

// ── Breadcrumbs ─────────────────────────────────────────

export interface BreadcrumbEntry {
  path: string;
  label: string;
}

// ── External open requests ──────────────────────────────

/**
 * "Show me this file" from outside the panel — a tool-card path click, or the
 * MCP editor-open event.
 *
 * The three fields travel together because a line without a path means nothing,
 * and because the request has to be able to repeat: clicking the same path
 * twice, with the user having browsed elsewhere in between, is two requests.
 * `seq` is what makes the second one distinguishable from the first.
 */
export interface FileOpenRequest {
  readonly path: string;
  /** 1-based line to jump to, or null to open at the top. */
  readonly line: number | null;
  /** Monotonic per request, so a repeat of the same path still re-reveals it. */
  readonly seq: number;
}

// ── Component props ─────────────────────────────────────

export interface CodeEditorPanelProps {
  focused?: boolean;
  /** File to reveal, set by whoever mounts the panel. */
  open?: FileOpenRequest | null;
  /** Project path — when this changes, the editor resets to the new project root */
  projectPath?: string | null;
  /** Active session used for editor LSP integration */
  sessionId?: string | null;
  /** Callback for surfacing errors to the user (e.g. toast) */
  onError?: (message: string) => void;
  /**
   * The file the panel is now showing, whenever that changes.
   *
   * The counterpart to `open`: that is what the panel was *asked* to reveal,
   * this is where it actually is. A pane's history needs the second, because
   * most files are reached by clicking the tree rather than by a request.
   */
  onActiveFileChanged?: (path: string | null) => void;
  /**
   * Which layout to render. Each mount site knows what it is — the desktop
   * side panel or the mobile sheet — so it says so rather than letting the
   * panel re-guess from the viewport and disagree with the CSS that decided
   * whether it is visible at all. Omitted, the viewport decides.
   */
  layout?: "desktop" | "mobile";
}

// ── Helpers ─────────────────────────────────────────────

export function isPreviewableRenderType(rt: FileRenderType): boolean {
  return rt === "markdown" || rt === "html" || rt === "mermaid" || rt === "svg";
}

export function isBinaryRenderType(rt: FileRenderType): boolean {
  return rt === "binary" || rt === "image" || rt === "audio" || rt === "video" || rt === "pdf" || rt === "model3d";
}

export function isDocRenderType(rt: FileRenderType): boolean {
  return rt === "spreadsheet" || rt === "document";
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// ── Re-exports ──────────────────────────────────────────

export type { FileReadResponse, FileRenderType, FileEntry, EditorLspDiagnostic, DocData, SheetData };
