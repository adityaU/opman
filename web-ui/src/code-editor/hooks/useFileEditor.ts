/**
 * useFileEditor — manages language loading, CodeMirror extensions,
 * view mode toggling, cursor tracking, line jumping, and the desktop breakpoint.
 */
import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { EditorView } from "@codemirror/view";
import { EditorSelection } from "@codemirror/state";
import { loadLanguageExtension, editorThemeExtension } from "../theme";
import type { OpenFileEntry, EditorViewMode } from "../types";

export interface FileEditorState {
  languageExtension: any;
  languageLoading: boolean;
  extensions: any[];
  viewModes: Record<string, EditorViewMode>;
  activeView: EditorViewMode;
  setActiveView: (mode: EditorViewMode) => void;
  cursorLine: number;
  cursorCol: number;
  setCursorLine: (n: number) => void;
  setCursorCol: (n: number) => void;
  jumpToLine: (line: number) => void;
  editorViewRef: React.MutableRefObject<any>;
  isDesktop: boolean;
}

// ── Desktop breakpoint ──────────────────────────────────

function useIsDesktop(breakpoint = 768) {
  const [isDesktop, setIsDesktop] = useState(
    typeof window !== "undefined" ? window.innerWidth >= breakpoint : false,
  );
  useEffect(() => {
    const mq = window.matchMedia(`(min-width: ${breakpoint}px)`);
    const handler = (e: MediaQueryListEvent) => setIsDesktop(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [breakpoint]);
  return isDesktop;
}

// ── Hook ────────────────────────────────────────────────

export function useFileEditor(
  activeFilePath: string | null,
  activeEntry: OpenFileEntry | null,
): FileEditorState {
  const isDesktop = useIsDesktop();
  const [languageExtension, setLanguageExtension] = useState<any>(null);
  const [languageLoading, setLanguageLoading]     = useState(false);
  const [viewModes, setViewModes]                 = useState<Record<string, EditorViewMode>>({});
  const [cursorLine, setCursorLine]               = useState(1);
  const [cursorCol, setCursorCol]                 = useState(1);
  const editorViewRef = useRef<any>(null);

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
    const exts = [EditorView.lineWrapping, ...editorThemeExtension];
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
