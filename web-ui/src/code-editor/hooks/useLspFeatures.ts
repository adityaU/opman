import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import {
  fetchEditorDiagnostics,
  fetchEditorDefinition,
  fetchEditorHover,
  fetchEditorCompletion,
  fetchEditorReferences,
  renameEditorSymbol,
  formatEditorFile,
  type EditorLspDiagnostic,
  type EditorCompletionResponse,
  type EditorReferenceLocation,
} from "../../api";
import type { OpenFileEntry } from "../types";
import { isBinaryRenderType } from "../types";

/** Quiet period after the last edit before diagnostics are re-requested. */
const DIAGNOSTIC_DEBOUNCE_MS = 450;
/** A cold server publishes nothing for a while; keep asking rather than
 *  reporting a confidently clean file. */
const COLD_POLL_MS = 700;
const COLD_POLL_LIMIT = 14;

export interface LspState {
  diagnostics: EditorLspDiagnostic[];
  activeDiagnostics: EditorLspDiagnostic[];
  hoverText: string | null;
  lspAvailable: boolean;
  lspBusy: null | "hover" | "definition" | "format";
  handleHover: () => Promise<void>;
  handleDefinition: () => Promise<void>;
  handleFormatWithLsp: () => Promise<void>;
  /** Hover text at an explicit position — used by the in-editor tooltip. */
  hoverAt: (line: number, col: number) => Promise<string | null>;
  /** Jump to the definition at an explicit position. Resolves true on a jump. */
  definitionAt: (line: number, col: number) => Promise<boolean>;
  /** Completions at an explicit position, for the editor's autocomplete. */
  completeAt: (
    line: number,
    col: number,
    trigger?: string,
  ) => Promise<EditorCompletionResponse | null>;
  /** Trigger characters the active server reported, e.g. `.` and `:`. */
  triggerCharacters: () => string[];
  /** Every use of the symbol at a position. */
  referencesAt: (line: number, col: number) => Promise<readonly EditorReferenceLocation[]>;
  /** Rename the symbol at a position across the project. */
  renameAt: (line: number, col: number, name: string) => Promise<boolean>;
  /** Open a file at a line — how a reference row navigates. */
  jumpTo: (file: string, line: number) => void;
}

export function useLspFeatures(
  activeEntry: OpenFileEntry | null,
  activeFilePath: string | null,
  sessionId: string | null | undefined,
  currentContent: string,
  cursorLine: number,
  cursorCol: number,
  loadFile: (path: string, line?: number | null) => Promise<void>,
  setOpenFiles: React.Dispatch<React.SetStateAction<OpenFileEntry[]>>,
  setSaveStatus: (s: "saved" | "modified" | null) => void,
  onError?: (msg: string) => void,
): LspState {
  const [diagnostics, setDiagnostics] = useState<EditorLspDiagnostic[]>([]);
  const [hoverText, setHoverText]     = useState<string | null>(null);
  const [lspAvailable, setLspAvailable] = useState(false);
  const [lspBusy, setLspBusy]         = useState<null | "hover" | "definition" | "format">(null);

  // Re-check diagnostics as the file changes, but not on every keystroke: the
  // request now carries the whole buffer, and a language server asked to
  // re-analyse mid-word answers about a half-typed identifier anyway.
  useEffect(() => {
    if (!activeEntry || !sessionId || isBinaryRenderType(activeEntry.renderType)) {
      setDiagnostics([]);
      setLspAvailable(false);
      return;
    }
    let cancelled = false;
    let attempts = 0;
    let timer = setTimeout(ask, DIAGNOSTIC_DEBOUNCE_MS);
    function ask(): void {
      fetchEditorDiagnostics(activeEntry!.path, sessionId!, currentContent)
        .then((resp) => {
          if (cancelled) return;
          setDiagnostics(resp.diagnostics ?? []);
          setLspAvailable(resp.available);
          // `published: false` means the server has not spoken about this file
          // yet — which is not the same as the file being clean.
          if (!resp.available || resp.published !== false || attempts >= COLD_POLL_LIMIT) return;
          attempts += 1;
          timer = setTimeout(ask, COLD_POLL_MS);
        })
        .catch(() => {
          if (cancelled) return;
          setDiagnostics([]);
          setLspAvailable(false);
        });
    }
    return () => { cancelled = true; clearTimeout(timer); };
  }, [activeEntry, sessionId, currentContent]);

  const activeDiagnostics = useMemo(
    () => diagnostics.filter((d) =>
      activeFilePath && (d.file.endsWith(activeFilePath) || d.file === activeFilePath),
    ),
    [diagnostics, activeFilePath],
  );

  // ── Position-addressed variants, for the in-editor affordances ──
  // These return their result instead of parking it in state: a tooltip that
  // follows the pointer cannot wait on a re-render to know what to show.

  const hoverAt = useCallback(async (line: number, col: number) => {
    if (!activeEntry || !sessionId) return null;
    try {
      const resp = await fetchEditorHover(activeEntry.path, sessionId, line, col, currentContent);
      setLspAvailable(resp.available);
      return resp.hover?.trim() || null;
    } catch {
      return null;
    }
  }, [activeEntry, sessionId, currentContent]);

  const definitionAt = useCallback(async (line: number, col: number) => {
    if (!activeEntry || !sessionId) return false;
    setLspBusy("definition");
    try {
      const resp = await fetchEditorDefinition(activeEntry.path, sessionId, line, col, currentContent);
      setLspAvailable(resp.available);
      const first = resp.locations?.[0];
      if (!first) { onError?.("No definition found here"); return false; }
      await loadFile(locationPath(first.file, activeFilePath), first.lnum);
      return true;
    } catch {
      onError?.("Definition lookup unavailable");
      return false;
    } finally {
      setLspBusy(null);
    }
  }, [activeEntry, activeFilePath, sessionId, currentContent, loadFile, onError]);

  // The server's trigger characters arrive with each completion response;
  // holding them in a ref keeps the CodeMirror extension stable while still
  // letting it ask for the current set.
  const triggersRef = useRef<string[]>([]);

  const completeAt = useCallback(async (line: number, col: number, trigger?: string) => {
    if (!activeEntry || !sessionId) return null;
    try {
      const resp = await fetchEditorCompletion(
        activeEntry.path, sessionId, line, col, currentContent, trigger,
      );
      if (resp.triggerCharacters?.length) triggersRef.current = resp.triggerCharacters;
      return resp;
    } catch {
      return null;
    }
  }, [activeEntry, sessionId, currentContent]);

  const triggerCharacters = useCallback(() => triggersRef.current, []);

  const referencesAt = useCallback(async (line: number, col: number): Promise<readonly EditorReferenceLocation[]> => {
    if (!activeEntry || !sessionId) return [];
    try {
      const resp = await fetchEditorReferences(activeEntry.path, sessionId, line, col, currentContent);
      setLspAvailable(resp.available);
      return resp.locations ?? [];
    } catch {
      return [];
    }
  }, [activeEntry, sessionId, currentContent]);

  const renameAt = useCallback(async (line: number, col: number, name: string) => {
    if (!activeEntry || !sessionId) return false;
    try {
      const resp = await renameEditorSymbol(activeEntry.path, sessionId, line, col, name, currentContent);
      setLspAvailable(resp.available);
      if (!resp.renamed) { onError?.("Nothing to rename here"); return false; }
      // The rename wrote through opman, so the open buffer is now behind disk.
      await loadFile(activeEntry.path, null);
      return true;
    } catch {
      onError?.("Rename unavailable");
      return false;
    }
  }, [activeEntry, sessionId, currentContent, loadFile, onError]);

  const jumpTo = useCallback((file: string, line: number) => {
    void loadFile(locationPath(file, activeFilePath), line);
  }, [activeFilePath, loadFile]);

  const handleHover = useCallback(async () => {
    if (!activeEntry || !sessionId) return;
    setLspBusy("hover");
    try {
      const resp = await fetchEditorHover(activeEntry.path, sessionId, cursorLine, cursorCol, currentContent);
      setLspAvailable(resp.available);
      setHoverText(resp.hover || "No hover information available at cursor.");
    } catch {
      setHoverText("Hover information unavailable.");
    } finally {
      setLspBusy(null);
    }
  }, [activeEntry, sessionId, cursorLine, cursorCol, currentContent]);

  const handleDefinition = useCallback(async () => {
    if (!activeEntry || !sessionId) return;
    setLspBusy("definition");
    try {
      const resp = await fetchEditorDefinition(activeEntry.path, sessionId, cursorLine, cursorCol, currentContent);
      setLspAvailable(resp.available);
      const first = resp.locations?.[0];
      if (first) await loadFile(locationPath(first.file, activeFilePath), first.lnum);
      else onError?.("No definition found at cursor");
    } catch {
      onError?.("Definition lookup unavailable");
    } finally {
      setLspBusy(null);
    }
  }, [activeEntry, activeFilePath, sessionId, cursorLine, cursorCol, currentContent, loadFile, onError]);

  const handleFormatWithLsp = useCallback(async () => {
    if (!activeEntry || !sessionId) return;
    setLspBusy("format");
    try {
      const resp = await formatEditorFile(activeEntry.path, sessionId, currentContent);
      setLspAvailable(resp.available);
      if (resp.formatted) {
        setOpenFiles((prev) =>
          prev.map((f) =>
            f.path === activeEntry.path ? { ...f, content: resp.content, editedContent: null } : f,
          ),
        );
        setSaveStatus("saved");
        setTimeout(() => setSaveStatus(null), 1500);
      }
    } catch {
      onError?.("LSP format unavailable for this file/session");
    } finally {
      setLspBusy(null);
    }
  }, [activeEntry, sessionId, currentContent, onError, setOpenFiles, setSaveStatus]);

  return {
    diagnostics, activeDiagnostics, hoverText,
    lspAvailable, lspBusy,
    handleHover, handleDefinition, handleFormatWithLsp,
    hoverAt, definitionAt, completeAt, triggerCharacters,
    referencesAt, renameAt, jumpTo,
  };
}

function locationPath(path: string, activePath: string | null): string {
  const normalizedActive = activePath?.replace(/^\.\//, "");
  if (!normalizedActive || path === activePath || path.endsWith(`/${normalizedActive}`)) return activePath ?? path;
  return path;
}
