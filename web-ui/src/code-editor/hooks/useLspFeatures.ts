import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import {
  fetchEditorDiagnostics,
  fetchEditorDefinition,
  fetchEditorHover,
  fetchEditorCompletion,
  fetchEditorReferences,
  renameEditorSymbol,
  formatEditorFile,
  readFile,
  type EditorGotoKind,
  type EditorDefinitionLocation,
  type EditorHoverResponse,
  type EditorLspDiagnostic,
  type EditorCompletionResponse,
  type EditorReferenceLocation,
} from "../../api";
import type { OpenFileEntry } from "../types";
import { useEditorOpener } from "../../tool-call/EditorOpenContext";
import { isBinaryRenderType } from "../types";

/** What an empty navigation result is called when reporting it. */
const GOTO_LABEL: Readonly<Record<EditorGotoKind, string>> = {
  "definition": "definition",
  "type-definition": "type definition",
  "implementation": "implementation",
  "declaration": "declaration",
};

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
  /** Hover at an explicit position — text plus what else this server can do. */
  hoverAt: (line: number, col: number) => Promise<EditorHoverResponse | null>;
  /** Jump to the definition at an explicit position. Resolves true on a jump. */
  definitionAt: (line: number, col: number) => Promise<boolean>;
  /** Resolve a navigation without acting on it. */
  gotoAt: (
    kind: EditorGotoKind,
    line: number,
    col: number,
  ) => Promise<readonly EditorDefinitionLocation[]>;
  /** Show a resolved location, here or in a pane the user picks. */
  reveal: (file: string, line: number, where: "here" | "choose", label: string) => void;
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
  projectPath: string | null | undefined,
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
  const opener = useEditorOpener();

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
      const hover = resp.hover?.trim() || null;
      return hover ? { ...resp, hover } : null;
    } catch {
      return null;
    }
  }, [activeEntry, sessionId, currentContent]);

  /**
   * Resolve a navigation without acting on it. The hover card needs the answer
   * before it can decide between jumping, listing, and asking for a pane.
   */
  const gotoAt = useCallback(
    async (kind: EditorGotoKind, line: number, col: number) => {
      if (!activeEntry || !sessionId) return [];
      try {
        const resp = await fetchEditorDefinition(
          activeEntry.path, sessionId, line, col, currentContent, kind,
        );
        setLspAvailable(resp.available);
        const locations = resp.locations ?? [];
        if (locations.length === 0) onError?.(`No ${GOTO_LABEL[kind]} found here`);
        return locations;
      } catch {
        onError?.(`${GOTO_LABEL[kind]} lookup unavailable`);
        return [];
      }
    },
    [activeEntry, sessionId, currentContent, onError],
  );

  const definitionAt = useCallback(async (line: number, col: number) => {
    if (!activeEntry || !sessionId) return false;
    setLspBusy("definition");
    try {
      const resp = await fetchEditorDefinition(activeEntry.path, sessionId, line, col, currentContent);
      setLspAvailable(resp.available);
      const first = resp.locations?.[0];
      if (!first) { onError?.("No definition found here"); return false; }
      await loadFile(locationPath(first.file, projectPath), first.lnum);
      return true;
    } catch {
      onError?.("Definition lookup unavailable");
      return false;
    } finally {
      setLspBusy(null);
    }
  }, [activeEntry, projectPath, sessionId, currentContent, loadFile, onError]);

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
      // `loadFile` returns early for a file that is already open, so the reread
      // has to be explicit or the editor keeps showing the old name.
      const fresh = await readFile(activeEntry.path);
      setOpenFiles((prev) => prev.map((file) => file.path === activeEntry.path
        ? { ...file, content: fresh.content, editedContent: null }
        : file));
      setSaveStatus(null);
      return true;
    } catch {
      onError?.("Rename unavailable");
      return false;
    }
  }, [activeEntry, sessionId, currentContent, onError, setOpenFiles, setSaveStatus]);

  const jumpTo = useCallback((file: string, line: number) => {
    void loadFile(locationPath(file, projectPath), line);
  }, [projectPath, loadFile]);

  /**
   * Show a resolved location. `here` replaces what the reader is looking at;
   * `choose` hands the question to the workspace's pane overlay, which is the
   * same one a session click uses. Without a workspace mounted — mobile, or the
   * board — there is only one place it can go, so it goes there.
   */
  const reveal = useCallback(
    (file: string, line: number, where: "here" | "choose", label: string) => {
      const path = locationPath(file, projectPath);
      if (where === "here" || !opener) {
        void loadFile(path, line);
        return;
      }
      opener.openWhere(path, line, label);
    },
    [projectPath, loadFile, opener],
  );

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
      if (first) await loadFile(locationPath(first.file, projectPath), first.lnum);
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
    hoverAt, definitionAt, gotoAt, completeAt, triggerCharacters,
    referencesAt, renameAt, jumpTo, reveal,
  };
}

/**
 * A language server answers with absolute paths; the explorer addresses files
 * relative to the project root. Translating between them needs the root.
 *
 * The previous rule folded any target whose tail matched the open file onto that
 * file — so jumping from `index.ts` to `lib/index.ts` silently stayed put, and
 * the header never changed. Two files sharing a basename is the normal case in
 * a real tree, not an edge one.
 */
function locationPath(path: string, projectPath: string | null | undefined): string {
  if (!projectPath) return path;
  if (path === projectPath) return path;
  const root = projectPath.endsWith("/") ? projectPath : `${projectPath}/`;
  return path.startsWith(root) ? path.slice(root.length) : path;
}
