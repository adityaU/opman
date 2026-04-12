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

// ── Component props ─────────────────────────────────────

export interface CodeEditorPanelProps {
  focused?: boolean;
  /** External file path to open (e.g. from chat tool calls) */
  openFilePath?: string | null;
  /** Optional line to jump to when opening or navigating a file */
  openLine?: number | null;
  /** Project path — when this changes, the editor resets to the new project root */
  projectPath?: string | null;
  /** Active session used for editor LSP integration */
  sessionId?: string | null;
  /** Callback for surfacing errors to the user (e.g. toast) */
  onError?: (message: string) => void;
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
