import { useState, useCallback, useEffect, useMemo } from "react";
import {
  fetchEditorDiagnostics,
  fetchEditorDefinition,
  fetchEditorHover,
  formatEditorFile,
  type EditorLspDiagnostic,
} from "../../api";
import type { OpenFileEntry } from "../types";
import { isBinaryRenderType } from "../types";

export interface LspState {
  diagnostics: EditorLspDiagnostic[];
  activeDiagnostics: EditorLspDiagnostic[];
  hoverText: string | null;
  lspAvailable: boolean;
  lspBusy: null | "hover" | "definition" | "format";
  handleHover: () => Promise<void>;
  handleDefinition: () => Promise<void>;
  handleFormatWithLsp: () => Promise<void>;
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

  // Fetch diagnostics when file/session/content changes
  useEffect(() => {
    if (!activeEntry || !sessionId) {
      setDiagnostics([]); setLspAvailable(false); return;
    }
    if (isBinaryRenderType(activeEntry.renderType)) {
      setDiagnostics([]); setLspAvailable(false); return;
    }
    fetchEditorDiagnostics(activeEntry.path, sessionId)
      .then((resp) => { setDiagnostics(resp.diagnostics ?? []); setLspAvailable(resp.available); })
      .catch(() => { setDiagnostics([]); setLspAvailable(false); });
  }, [activeEntry, sessionId, currentContent]);

  const activeDiagnostics = useMemo(
    () => diagnostics.filter((d) =>
      activeFilePath && (d.file.endsWith(activeFilePath) || d.file === activeFilePath),
    ),
    [diagnostics, activeFilePath],
  );

  const handleHover = useCallback(async () => {
    if (!activeEntry || !sessionId) return;
    setLspBusy("hover");
    try {
      const resp = await fetchEditorHover(activeEntry.path, sessionId, cursorLine, cursorCol);
      setLspAvailable(resp.available);
      setHoverText(resp.hover || "No hover information available at cursor.");
    } catch {
      setHoverText("Hover information unavailable.");
    } finally {
      setLspBusy(null);
    }
  }, [activeEntry, sessionId, cursorLine, cursorCol]);

  const handleDefinition = useCallback(async () => {
    if (!activeEntry || !sessionId) return;
    setLspBusy("definition");
    try {
      const resp = await fetchEditorDefinition(activeEntry.path, sessionId, cursorLine, cursorCol);
      setLspAvailable(resp.available);
      const first = resp.locations?.[0];
      if (first) await loadFile(first.file, first.lnum);
      else onError?.("No definition found at cursor");
    } catch {
      onError?.("Definition lookup unavailable");
    } finally {
      setLspBusy(null);
    }
  }, [activeEntry, sessionId, cursorLine, cursorCol, loadFile, onError]);

  const handleFormatWithLsp = useCallback(async () => {
    if (!activeEntry || !sessionId) return;
    setLspBusy("format");
    try {
      const resp = await formatEditorFile(activeEntry.path, sessionId);
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
  }, [activeEntry, sessionId, onError, setOpenFiles, setSaveStatus]);

  return {
    diagnostics, activeDiagnostics, hoverText,
    lspAvailable, lspBusy,
    handleHover, handleDefinition, handleFormatWithLsp,
  };
}
