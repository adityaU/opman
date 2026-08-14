/**
 * useFileEditor — manages language loading, CodeMirror extensions,
 * view mode toggling, cursor tracking, line jumping, and the desktop breakpoint.
 */
import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { EditorView } from "@codemirror/view";
import { EditorSelection } from "@codemirror/state";
import type { Extension } from "@codemirror/state";
import { loadLanguageExtension, editorThemeExtension } from "../theme";
import { foldGutterExtension } from "../fold";
import type { OpenFileEntry, EditorViewMode } from "../types";
import { useIsMobile } from "../../hooks/useIsMobile";

export interface FileEditorState {
  languageExtension: Extension | null;
  languageLoading: boolean;
  extensions: Extension[];
  viewModes: Record<string, EditorViewMode>;
  activeView: EditorViewMode;
  setActiveView: (mode: EditorViewMode) => void;
  cursorLine: number;
  cursorCol: number;
  setCursorLine: (n: number) => void;
  setCursorCol: (n: number) => void;
  jumpToLine: (line: number) => void;
  editorViewRef: React.MutableRefObject<EditorView | null>;
  isDesktop: boolean;
}

// ── Hook ────────────────────────────────────────────────

export function useFileEditor(
  activeFilePath: string | null,
  activeEntry: OpenFileEntry | null,
): FileEditorState {
  // Shared with the rest of the app, and phrased as the stylesheets phrase it.
  const isDesktop = !useIsMobile();
  const [languageExtension, setLanguageExtension] = useState<Extension | null>(null);
  const [languageLoading, setLanguageLoading]     = useState(false);
  const [viewModes, setViewModes]                 = useState<Record<string, EditorViewMode>>({});
  const [cursorLine, setCursorLine]               = useState(1);
  const [cursorCol, setCursorCol]                 = useState(1);
  const editorViewRef = useRef<EditorView | null>(null);

  const openFile = activeEntry
    ? { path: activeEntry.path, content: activeEntry.content, language: activeEntry.language }
    : null;

  const activeView = activeFilePath ? viewModes[activeFilePath] ?? "code" : "code";

  const setActiveView = useCallback((mode: EditorViewMode) => {
    if (!activeFilePath) return;
    setViewModes((prev) => ({ ...prev, [activeFilePath]: mode }));
  }, [activeFilePath]);

  // Language extension loading
  useEffect(() => {
    let cancelled = false;
    if (!openFile) { setLanguageExtension(null); return; }
    setLanguageLoading(true);
    loadLanguageExtension(openFile.path, openFile.language)
      .then((ext) => { if (!cancelled) setLanguageExtension(ext); })
      .finally(() => { if (!cancelled) setLanguageLoading(false); });
    return () => { cancelled = true; };
  }, [openFile?.path, openFile?.language]);

  // Build extensions
  const extensions = useMemo(() => {
    const exts: Extension[] = [EditorView.lineWrapping, ...editorThemeExtension, foldGutterExtension];
    if (languageExtension) exts.push(languageExtension);
    return exts;
  }, [languageExtension]);

  // Jump to line
  const jumpToLine = useCallback((line: number) => {
    const view = editorViewRef.current;
    if (!view || !view.state?.doc) return;
    const targetLine = Math.max(1, Math.min(line, view.state.doc.lines));
    const lineInfo = view.state.doc.line(targetLine);
    view.dispatch({ selection: EditorSelection.cursor(lineInfo.from), scrollIntoView: true });
    view.focus();
  }, []);

  return {
    languageExtension, languageLoading, extensions,
    viewModes, activeView, setActiveView,
    cursorLine, cursorCol, setCursorLine, setCursorCol,
    jumpToLine, editorViewRef, isDesktop,
  };
}
