/**
 * CodeEditorPanel — desktop-friendly code editor using CodeMirror 6.
 *
 * Features:
 * - Desktop mode: file explorer (left) + editor (right) side by side
 * - Mobile mode: file browser navigates to editor (full screen), with back button
 * - Touch input, syntax highlighting, and direct file editing
 * - External file opening via `openFilePath` prop (e.g. from chat tool calls)
 */
import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { LanguageDescription, HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { languages as languageData } from "@codemirror/language-data";
import { EditorView } from "@codemirror/view";
import { EditorSelection } from "@codemirror/state";
import { tags } from "@lezer/highlight";
import DOMPurify from "dompurify";
import mermaid from "mermaid";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Folder,
  File,
  ChevronLeft,
  ChevronRight,
  ChevronDown,
  Save,
  RotateCcw,
  Loader2,
  PanelLeftClose,
  PanelLeftOpen,
  X,
  Eye,
  Code2,
  AlertCircle,
  Wand2,
  Info,
  ArrowRightCircle,
} from "lucide-react";
import {
  browseFiles,
  readFile,
  writeFile,
  rawFileUrl,
  classifyFile,
  type FileEntry,
  type FileReadResponse,
  type FileRenderType,
  fetchEditorDiagnostics,
  fetchEditorDefinition,
  fetchEditorHover,
  formatEditorFile,
  type EditorLspDiagnostic,
} from "./api";

// ── Language extension loading ───────────────────────────

async function loadLanguageExtension(path: string, lang: string) {
  const byPath = LanguageDescription.matchFilename(languageData, path);
  if (byPath) return byPath.load();
  const normalized = lang.toLowerCase();
  const byName = languageData.find((entry) =>
    entry.name.toLowerCase() === normalized || entry.alias.includes(normalized)
  );
  return byName ? byName.load() : null;
}

type EditorViewMode = "code" | "rendered";

function isPreviewableRenderType(renderType: FileRenderType) {
  return renderType === "markdown" || renderType === "html" || renderType === "mermaid" || renderType === "svg";
}

// ── Custom CodeMirror theme (uses CSS variables from app theme) ──

/** Editor chrome — background, gutters, selection, cursor, etc. */
const editorTheme = EditorView.theme(
  {
    "&": {
      height: "100%",
      fontSize: "13px",
      backgroundColor: "var(--color-bg)",
      color: "var(--color-text)",
    },
    ".cm-scroller": {
      overflow: "auto",
      fontFamily: "var(--font-mono, monospace)",
    },
    ".cm-content": {
      caretColor: "var(--color-primary)",
    },
    ".cm-cursor, .cm-dropCursor": {
      borderLeftColor: "var(--color-primary)",
    },
    ".cm-gutters": {
      backgroundColor: "var(--color-bg)",
      color: "var(--color-text-muted)",
      borderRight: "1px solid var(--color-border-subtle)",
    },
    ".cm-activeLineGutter": {
      backgroundColor: "var(--theme-surface-hover, var(--color-bg-hover))",
      color: "var(--color-text)",
    },
    ".cm-activeLine": {
      backgroundColor: "var(--theme-surface-3, var(--color-bg-element))",
    },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
      {
        backgroundColor: "var(--color-bg-element, #1a1a1a)",
      },
    ".cm-selectionMatch": {
      backgroundColor: "var(--theme-surface-hover, var(--color-bg-hover))",
    },
    ".cm-matchingBracket": {
      backgroundColor: "var(--theme-surface-hover, var(--color-bg-hover))",
      outline: "1px solid var(--color-border)",
    },
    ".cm-foldGutter .cm-gutterElement": {
      color: "var(--color-text-muted)",
    },
    ".cm-foldPlaceholder": {
      backgroundColor: "var(--color-bg-element)",
      border: "1px solid var(--color-border-subtle)",
      color: "var(--color-text-muted)",
    },
    ".cm-tooltip": {
      backgroundColor: "var(--color-bg-panel)",
      border: "1px solid var(--color-border)",
      color: "var(--color-text)",
    },
    ".cm-tooltip-autocomplete": {
      "& > ul > li[aria-selected]": {
        backgroundColor: "var(--color-bg-element)",
      },
    },
    ".cm-searchMatch": {
      backgroundColor: "var(--theme-primary-soft)",
      outline: "1px solid var(--theme-primary-border)",
    },
    ".cm-searchMatch.cm-searchMatch-selected": {
      backgroundColor: "color-mix(in srgb, var(--color-primary) 18%, var(--color-bg-element))",
    },
  },
  { dark: true }
);

/**
 * Syntax highlighting — maps lezer highlight tags to the same
 * CSS variables used by the chat code blocks (Prism), keeping
 * colors consistent across the whole UI.
 */
const editorHighlightStyle = HighlightStyle.define([
  // Keywords: if, else, return, const, let, fn, pub, etc.
  { tag: [tags.keyword, tags.modifier, tags.operatorKeyword],
    color: "var(--color-syntax-keyword, var(--color-primary))" },

  // Functions and methods
  { tag: [tags.function(tags.variableName), tags.function(tags.definition(tags.variableName))],
    color: "var(--color-syntax-function, var(--color-accent))" },

  // Definitions (class names, type names)
  { tag: [tags.definition(tags.typeName), tags.typeName, tags.className, tags.namespace],
    color: "var(--color-syntax-tag, var(--color-secondary))" },

  // Strings and template literals
  { tag: [tags.string, tags.special(tags.string), tags.character],
    color: "var(--color-syntax-string, var(--color-success))" },

  // Numbers and booleans
  { tag: [tags.number, tags.integer, tags.float, tags.bool],
    color: "var(--color-syntax-number, var(--color-warning))" },

  // Comments
  { tag: [tags.comment, tags.lineComment, tags.blockComment],
    color: "var(--color-syntax-comment, var(--color-text-muted))",
    fontStyle: "italic" },

  // Operators
  { tag: [tags.operator, tags.compareOperator, tags.arithmeticOperator, tags.logicOperator, tags.updateOperator],
    color: "var(--color-syntax-operator, var(--color-text))" },

  // Punctuation (braces, parens, brackets, commas, semicolons)
  { tag: [tags.punctuation, tags.separator, tags.bracket, tags.angleBracket, tags.squareBracket, tags.paren, tags.brace],
    color: "var(--color-syntax-punctuation, var(--color-text-muted))" },

  // HTML/XML tags
  { tag: [tags.tagName, tags.standard(tags.tagName)],
    color: "var(--color-syntax-tag, var(--color-secondary))" },

  // Attributes (HTML/JSX)
  { tag: [tags.attributeName],
    color: "var(--color-syntax-attribute, var(--color-info))" },

  // Attribute values
  { tag: [tags.attributeValue],
    color: "var(--color-syntax-string, var(--color-success))" },

  // Regex
  { tag: [tags.regexp],
    color: "var(--color-syntax-regex, var(--color-error))" },

  // Variable names (generic)
  { tag: [tags.variableName],
    color: "var(--color-text)" },

  // Property names (object.prop, struct fields)
  { tag: [tags.propertyName, tags.definition(tags.propertyName)],
    color: "var(--color-syntax-attribute, var(--color-info))" },

  // Special: self, this, null, undefined
  { tag: [tags.self, tags.null],
    color: "var(--color-syntax-keyword, var(--color-primary))" },

  // Escape sequences inside strings
  { tag: [tags.escape],
    color: "var(--color-syntax-regex, var(--color-error))" },

  // Heading (markdown)
  { tag: [tags.heading],
    color: "var(--color-primary)",
    fontWeight: "bold" },

  // Links (markdown)
  { tag: [tags.link, tags.url],
    color: "var(--color-secondary)",
    textDecoration: "underline" },

  // Emphasis (markdown)
  { tag: [tags.emphasis],
    fontStyle: "italic" },
  { tag: [tags.strong],
    fontWeight: "bold" },

  // Meta (preprocessor, annotations, decorators)
  { tag: [tags.meta, tags.annotation, tags.processingInstruction],
    color: "var(--color-syntax-comment, var(--color-text-muted))" },

  // Invalid / error
  { tag: [tags.invalid],
    color: "var(--color-error)",
    textDecoration: "underline wavy" },
]);

/** Combined theme extension: chrome + syntax highlighting. */
const editorThemeExtension = [editorTheme, syntaxHighlighting(editorHighlightStyle)];

// ── Desktop breakpoint ──────────────────────────────────

function useIsDesktop(breakpoint = 768) {
  const [isDesktop, setIsDesktop] = useState(
    typeof window !== "undefined" ? window.innerWidth >= breakpoint : false
  );
  useEffect(() => {
    const mq = window.matchMedia(`(min-width: ${breakpoint}px)`);
    const handler = (e: MediaQueryListEvent) => setIsDesktop(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [breakpoint]);
  return isDesktop;
}

// ── Component ───────────────────────────────────────────

interface Props {
  focused?: boolean;
  /** External file path to open (e.g. from chat tool calls) */
  openFilePath?: string | null;
  /** Optional line to jump to when opening or navigating a file. */
  openLine?: number | null;
  /** Project path — when this changes, the editor resets to the new project root */
  projectPath?: string | null;
  /** Active session used for editor LSP integration. */
  sessionId?: string | null;
  /** Callback for surfacing errors to the user (e.g. toast) */
  onError?: (message: string) => void;
}

/** Tracks an open file with its content and unsaved edits. */
interface OpenFileEntry {
  path: string;
  content: string;
  language: string;
  renderType: FileRenderType;
  /** Non-null when the file has unsaved edits */
  editedContent: string | null;
}

interface BreadcrumbEntry {
  path: string;
  label: string;
}

export default function CodeEditorPanel({ focused, openFilePath, openLine, projectPath, sessionId, onError }: Props) {
  const isDesktop = useIsDesktop();

  // File explorer state
  const [currentPath, setCurrentPath] = useState(".");
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [loadingDir, setLoadingDir] = useState(false);
  const [explorerCollapsed, setExplorerCollapsed] = useState(false);

  // Collapsible directories in explorer (desktop tree mode)
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [dirChildren, setDirChildren] = useState<Record<string, FileEntry[]>>({});
  const [loadingDirs, setLoadingDirs] = useState<Set<string>>(new Set());

  // Editor state
  const [openFiles, setOpenFiles] = useState<OpenFileEntry[]>([]);
  const [activeFilePath, setActiveFilePath] = useState<string | null>(null);
  const [loadingFile, setLoadingFile] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"saved" | "modified" | null>(
    null
  );
  const [languageExtension, setLanguageExtension] = useState<any>(null);
  const [languageLoading, setLanguageLoading] = useState(false);
  const [viewModes, setViewModes] = useState<Record<string, EditorViewMode>>({});
  const [cursorLine, setCursorLine] = useState(1);
  const [cursorCol, setCursorCol] = useState(1);
  const [diagnostics, setDiagnostics] = useState<EditorLspDiagnostic[]>([]);
  const [hoverText, setHoverText] = useState<string | null>(null);
  const [lspAvailable, setLspAvailable] = useState(false);
  const [lspBusy, setLspBusy] = useState<null | "hover" | "definition" | "format">(null);
  const pendingJumpRef = useRef<{ path: string; line: number } | null>(null);
  const editorViewRef = useRef<any>(null);
  const editorRef = useRef<HTMLDivElement>(null);

  // Derived: the currently active open file entry
  const activeEntry = openFiles.find((f) => f.path === activeFilePath) ?? null;
  // Compat: openFile shaped like FileReadResponse for the editor/toolbar
  const openFile: FileReadResponse | null = activeEntry
    ? { path: activeEntry.path, content: activeEntry.content, language: activeEntry.language }
    : null;
  const editedContent = activeEntry?.editedContent ?? null;
  const fileRenderType: FileRenderType = activeEntry?.renderType ?? "code";
  const activeView = activeFilePath ? viewModes[activeFilePath] ?? "code" : "code";

  // Breadcrumb navigation (for mobile file browser)
  const breadcrumbs: BreadcrumbEntry[] = useMemo(() => {
    const parts = currentPath === "." ? [] : currentPath.split("/");
    const crumbs: BreadcrumbEntry[] = [{ path: ".", label: "root" }];
    for (let i = 0; i < parts.length; i++) {
      crumbs.push({
        path: parts.slice(0, i + 1).join("/"),
        label: parts[i],
      });
    }
    return crumbs;
  }, [currentPath]);

  // Load directory listing
  const loadDirectory = useCallback(async (path: string) => {
    setLoadingDir(true);
    try {
      const resp = await browseFiles(path === "." ? undefined : path);
      setEntries(resp.entries);
      setCurrentPath(resp.path || ".");
    } catch (err) {
      console.error("Failed to browse files:", err);
      onError?.("Failed to load directory");
      setEntries([]);
    } finally {
      setLoadingDir(false);
    }
  }, []);

  // Load directory children for tree expansion
  const loadDirChildren = useCallback(async (dirPath: string) => {
    setLoadingDirs((prev) => new Set([...prev, dirPath]));
    try {
      const resp = await browseFiles(dirPath);
      setDirChildren((prev) => ({ ...prev, [dirPath]: resp.entries }));
    } catch (err) {
      console.error("Failed to browse directory:", err);
      onError?.("Failed to expand directory");
    } finally {
      setLoadingDirs((prev) => {
        const next = new Set(prev);
        next.delete(dirPath);
        return next;
      });
    }
  }, []);

  // Toggle directory expansion in tree
  const toggleDir = useCallback(
    (dirPath: string) => {
      setExpandedDirs((prev) => {
        const next = new Set(prev);
        if (next.has(dirPath)) {
          next.delete(dirPath);
        } else {
          next.add(dirPath);
          // Load children if not already loaded
          if (!dirChildren[dirPath]) {
            loadDirChildren(dirPath);
          }
        }
        return next;
      });
    },
    [dirChildren, loadDirChildren]
  );

  // Load file content (opens a new file or switches to an already-open one)
  const loadFile = useCallback(async (path: string, line?: number | null) => {
    // If already open, just switch to it
    const existing = openFiles.find((f) => f.path === path);
    if (existing) {
      setActiveFilePath(path);
      setSaveStatus(existing.editedContent !== null ? "modified" : null);
      if (line) pendingJumpRef.current = { path, line };
      return;
    }

    const renderType = classifyFile(path);
    setLoadingFile(true);
    try {
      let content = "";
      let language: string = renderType;

      if (renderType === "image" || renderType === "audio" || renderType === "video" || renderType === "pdf" || renderType === "binary") {
        // Binary types: stub content
      } else {
        const resp = await readFile(path);
        content = resp.content;
        language = resp.language;
      }

      const entry: OpenFileEntry = { path, content, language, renderType, editedContent: null };
      setOpenFiles((prev) => [...prev, entry]);
      setActiveFilePath(path);
      setSaveStatus(null);
      if (line) pendingJumpRef.current = { path, line };
    } catch (err) {
      console.error("Failed to read file:", err);
      onError?.("Failed to read file");
    } finally {
      setLoadingFile(false);
    }
  }, [openFiles, onError]);

  // Close a file (remove from open files list)
  const closeFile = useCallback((path: string) => {
    setOpenFiles((prev) => {
      const idx = prev.findIndex((f) => f.path === path);
      if (idx === -1) return prev;
      const next = prev.filter((f) => f.path !== path);
      // If closing the active file, switch to a neighbor
      if (path === activeFilePath) {
        if (next.length === 0) {
          setActiveFilePath(null);
          setSaveStatus(null);
        } else {
          // Prefer the file to the left, else the first remaining
          const neighbor = next[Math.min(idx, next.length - 1)];
          setActiveFilePath(neighbor.path);
          setSaveStatus(neighbor.editedContent !== null ? "modified" : null);
        }
      }
      return next;
    });
  }, [activeFilePath]);

  // Save file
  const handleSave = useCallback(async () => {
    if (!activeFilePath || editedContent === null) return;
    setSaving(true);
    try {
      await writeFile(activeFilePath, editedContent);
      setOpenFiles((prev) =>
        prev.map((f) =>
          f.path === activeFilePath
            ? { ...f, content: editedContent, editedContent: null }
            : f
        )
      );
      setSaveStatus("saved");
      setTimeout(() => setSaveStatus(null), 2000);
    } catch (err) {
      console.error("Failed to save file:", err);
      onError?.("Failed to save file");
    } finally {
      setSaving(false);
    }
  }, [activeFilePath, editedContent, onError]);

  // Revert changes
  const handleRevert = useCallback(() => {
    if (!activeFilePath) return;
    setOpenFiles((prev) =>
      prev.map((f) =>
        f.path === activeFilePath ? { ...f, editedContent: null } : f
      )
    );
    setSaveStatus(null);
  }, [activeFilePath]);

  // Initial directory load
  useEffect(() => {
    loadDirectory(".");
  }, [loadDirectory]);

  // Reset editor state when project changes (session switch)
  const prevProjectPath = useRef(projectPath);
  useEffect(() => {
    if (projectPath === prevProjectPath.current) return;
    prevProjectPath.current = projectPath;
    // Reset file data (preserve UI prefs like explorerCollapsed)
    setCurrentPath(".");
    setEntries([]);
    setExpandedDirs(new Set());
    setDirChildren({});
    setOpenFiles([]);
    setActiveFilePath(null);
    setSaveStatus(null);
    // Reload root directory for new project
    loadDirectory(".");
  }, [projectPath, loadDirectory]);

  // Handle external file open requests
  useEffect(() => {
    if (openFilePath) {
      loadFile(openFilePath, openLine ?? undefined);
    }
  }, [openFilePath, openLine, loadFile]);

  useEffect(() => {
    let cancelled = false;
    if (!openFile) {
      setLanguageExtension(null);
      return;
    }
    setLanguageLoading(true);
    loadLanguageExtension(openFile.path, openFile.language)
      .then((ext) => {
        if (!cancelled) setLanguageExtension(ext);
      })
      .finally(() => {
        if (!cancelled) setLanguageLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [openFile]);

  const isModified = editedContent !== null;
  const currentContent = isModified ? editedContent : openFile?.content || "";

  useEffect(() => {
    if (!activeEntry || !sessionId) {
      setDiagnostics([]);
      setLspAvailable(false);
      return;
    }
    if (activeEntry.renderType === "binary" || activeEntry.renderType === "image" || activeEntry.renderType === "audio" || activeEntry.renderType === "video" || activeEntry.renderType === "pdf") {
      setDiagnostics([]);
      setLspAvailable(false);
      return;
    }
    fetchEditorDiagnostics(activeEntry.path, sessionId)
      .then((resp) => {
        setDiagnostics(resp.diagnostics ?? []);
        setLspAvailable(resp.available);
      })
      .catch(() => {
        setDiagnostics([]);
        setLspAvailable(false);
      });
  }, [activeEntry, sessionId, currentContent]);

  // Keyboard shortcut: Cmd+S / Ctrl+S to save
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s" && openFile && focused) {
        e.preventDefault();
        handleSave();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleSave, openFile, focused]);

  // Handle content changes
  const onEditorChange = useCallback(
    (value: string) => {
      if (!activeFilePath || !activeEntry) return;
      if (value !== activeEntry.content) {
        setOpenFiles((prev) =>
          prev.map((f) =>
            f.path === activeFilePath ? { ...f, editedContent: value } : f
          )
        );
        setSaveStatus("modified");
      } else {
        setOpenFiles((prev) =>
          prev.map((f) =>
            f.path === activeFilePath ? { ...f, editedContent: null } : f
          )
        );
        setSaveStatus(null);
      }
    },
    [activeFilePath, activeEntry]
  );

  // Build CodeMirror extensions
  const extensions = useMemo(() => {
    const exts = [
      EditorView.lineWrapping,
      ...editorThemeExtension,
    ];
    if (languageExtension) exts.push(languageExtension);
    return exts;
  }, [languageExtension]);

  const activeDiagnostics = useMemo(
    () => diagnostics.filter((item) => activeFilePath && (item.file.endsWith(activeFilePath) || item.file === activeFilePath)),
    [diagnostics, activeFilePath]
  );

  const setActiveView = useCallback((mode: EditorViewMode) => {
    if (!activeFilePath) return;
    setViewModes((prev) => ({ ...prev, [activeFilePath]: mode }));
  }, [activeFilePath]);

  const jumpToLine = useCallback((line: number) => {
    const view = editorViewRef.current;
    if (!view || !view.state?.doc) return;
    const targetLine = Math.max(1, Math.min(line, view.state.doc.lines));
    const lineInfo = view.state.doc.line(targetLine);
    view.dispatch({
      selection: EditorSelection.cursor(lineInfo.from),
      scrollIntoView: true,
    });
    view.focus();
  }, []);

  useEffect(() => {
    const pending = pendingJumpRef.current;
    if (!pending || pending.path !== activeFilePath || activeView !== "code") return;
    jumpToLine(pending.line);
    pendingJumpRef.current = null;
  }, [activeFilePath, activeView, jumpToLine, currentContent]);

  const handleFormatWithLsp = useCallback(async () => {
    if (!activeEntry || !sessionId) return;
    setLspBusy("format");
    try {
      const resp = await formatEditorFile(activeEntry.path, sessionId);
      setLspAvailable(resp.available);
      if (resp.formatted) {
        setOpenFiles((prev) => prev.map((f) =>
          f.path === activeEntry.path ? { ...f, content: resp.content, editedContent: null } : f
        ));
        setSaveStatus("saved");
        setTimeout(() => setSaveStatus(null), 1500);
      }
    } catch {
      onError?.("LSP format unavailable for this file/session");
    } finally {
      setLspBusy(null);
    }
  }, [activeEntry, sessionId, onError]);

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
      if (first) {
        await loadFile(first.file, first.lnum);
      } else {
        onError?.("No definition found at cursor");
      }
    } catch {
      onError?.("Definition lookup unavailable");
    } finally {
      setLspBusy(null);
    }
  }, [activeEntry, sessionId, cursorLine, cursorCol, loadFile, onError]);

  // Navigate into a directory (mobile mode)
  const handleEntryClick = (entry: FileEntry) => {
    if (entry.is_dir) {
      loadDirectory(entry.path);
    } else {
      loadFile(entry.path);
    }
  };

  // Go back to file browser (mobile mode)
  const handleBackToBrowser = () => {
    setActiveFilePath(null);
    setSaveStatus(null);
  };

  // ── Shared sub-components ──────────────────────────────

  /** The editor toolbar (shown above the code area) */
  const editorToolbar = openFile ? (
    <div className="code-editor-toolbar">
      {!isDesktop && (
        <button
          className="code-editor-back"
          onClick={handleBackToBrowser}
          title="Back to files"
          aria-label="Back to files"
        >
          <ChevronLeft size={14} />
        </button>
      )}
      <span className="code-editor-filename" title={openFile.path}>
        {openFile.path}
      </span>
      {isModified && (
        <span className="code-editor-modified-dot" title="Unsaved changes">
          &bull;
        </span>
      )}
      <span className="code-editor-spacer" />
      {isPreviewableRenderType(fileRenderType) && (
        <div className="code-editor-view-tabs">
          <button
            className={`code-editor-view-tab ${activeView === "code" ? "active" : ""}`}
            onClick={() => setActiveView("code")}
          >
            <Code2 size={13} />
            Code
          </button>
          <button
            className={`code-editor-view-tab ${activeView === "rendered" ? "active" : ""}`}
            onClick={() => setActiveView("rendered")}
          >
            <Eye size={13} />
            Rendered
          </button>
        </div>
      )}
      {openFile && fileRenderType !== "binary" && fileRenderType !== "image" && fileRenderType !== "audio" && fileRenderType !== "video" && fileRenderType !== "pdf" && (
        <div className="code-editor-lsp-group">
          <span className={`code-editor-lsp-pill ${lspAvailable ? "active" : "inactive"}`}>
            <AlertCircle size={12} />
            {activeDiagnostics.length} issues
          </span>
          <button className="code-editor-action" onClick={handleHover} title="Hover info at cursor">
            {lspBusy === "hover" ? <Loader2 size={13} className="spin" /> : <Info size={13} />}
          </button>
          <button className="code-editor-action" onClick={handleDefinition} title="Go to definition">
            {lspBusy === "definition" ? <Loader2 size={13} className="spin" /> : <ArrowRightCircle size={13} />}
          </button>
          <button className="code-editor-action" onClick={handleFormatWithLsp} title="Format with LSP">
            {lspBusy === "format" ? <Loader2 size={13} className="spin" /> : <Wand2 size={13} />}
          </button>
        </div>
      )}
      {saveStatus === "saved" && (
        <span className="code-editor-save-status">Saved</span>
      )}
      {isModified && (
        <>
          <button
            className="code-editor-action"
            onClick={handleRevert}
            title="Revert changes"
            aria-label="Revert changes"
          >
            <RotateCcw size={13} />
          </button>
          <button
            className="code-editor-action code-editor-save"
            onClick={handleSave}
            disabled={saving}
            title="Save (Cmd+S)"
            aria-label="Save file"
          >
            {saving ? (
              <Loader2 size={13} className="spin" />
            ) : (
              <Save size={13} />
            )}
          </button>
        </>
      )}
    </div>
  ) : null;

  /** Render file content based on type */
  const renderFileContent = () => {
    if (!openFile) return null;
    const url = rawFileUrl(openFile.path);

    const renderEditor = () => (
      <CodeMirror
        value={currentContent}
        onChange={onEditorChange}
        onCreateEditor={(view) => {
          editorViewRef.current = view;
        }}
        onUpdate={(update) => {
          const pos = update.state.selection.main.head;
          const line = update.state.doc.lineAt(pos);
          setCursorLine(line.number);
          setCursorCol(pos - line.from + 1);
        }}
        extensions={extensions}
        theme="none"
        basicSetup={{
          lineNumbers: true,
          highlightActiveLineGutter: true,
          highlightActiveLine: true,
          foldGutter: true,
          bracketMatching: true,
          closeBrackets: true,
          autocompletion: true,
          indentOnInput: true,
        }}
      />
    );

    if (isPreviewableRenderType(fileRenderType) && activeView === "code") {
      return renderEditor();
    }

    switch (fileRenderType) {
      case "image":
        return (
          <div className="file-preview file-preview-image">
            <img src={url} alt={openFile.path} />
          </div>
        );

      case "audio":
        return (
          <div className="file-preview file-preview-audio">
            <div className="file-preview-icon">
              <File size={48} strokeWidth={1} />
            </div>
            <span className="file-preview-name">
              {openFile.path.split("/").pop()}
            </span>
            <audio controls src={url} preload="metadata">
              Your browser does not support the audio element.
            </audio>
          </div>
        );

      case "video":
        return (
          <div className="file-preview file-preview-video">
            <video controls src={url} preload="metadata">
              Your browser does not support the video element.
            </video>
          </div>
        );

      case "pdf":
        return (
          <div className="file-preview file-preview-pdf">
            <iframe src={url} title={openFile.path} />
          </div>
        );

      case "csv":
        return <CsvViewer content={openFile.content} />;

      case "markdown":
        return <MarkdownViewer content={currentContent} />;

      case "html":
        return <HtmlViewer content={currentContent} />;

      case "mermaid":
        return <MermaidViewer content={currentContent} />;

      case "svg":
        return <SvgViewer content={currentContent} />;

      case "binary":
        return (
          <div className="file-preview file-preview-binary">
            <File size={48} strokeWidth={1} />
            <span className="file-preview-label">
              Binary file — cannot be displayed
            </span>
            <span className="file-preview-name">
              {openFile.path.split("/").pop()}
            </span>
          </div>
        );

      case "code":
      default:
        return renderEditor();
    }
  };

  /** The editor body area */
  const editorBody = (
    <div className="code-editor-body">
      {loadingFile ? (
        <div className="code-editor-loading">
          <Loader2 size={20} className="spin" />
          <span>Loading...</span>
        </div>
      ) : openFile ? (
        <>
          {hoverText && (
            <div className="code-editor-hover-card">
              <div className="code-editor-hover-title">Hover</div>
              <pre>{hoverText}</pre>
            </div>
          )}
          {activeDiagnostics.length > 0 && (
            <div className="code-editor-diagnostics">
              {activeDiagnostics.slice(0, 6).map((diag, idx) => (
                <div key={`${diag.lnum}-${diag.col}-${idx}`} className={`code-editor-diagnostic severity-${diag.severity.toLowerCase()}`}>
                  <span className="code-editor-diagnostic-pos">L{diag.lnum}:C{diag.col}</span>
                  <span className="code-editor-diagnostic-msg">{diag.message}</span>
                </div>
              ))}
            </div>
          )}
          {languageLoading ? (
            <div className="code-editor-loading-inline">
              <Loader2 size={16} className="spin" />
              <span>Loading language tools...</span>
            </div>
          ) : null}
          {renderFileContent()}
        </>
      ) : (
        <div className="code-editor-empty-state">
          <File size={32} strokeWidth={1} />
          <span>Select a file to edit</span>
        </div>
      )}
    </div>
  );

  // ── Recursive tree node for desktop explorer ───────────

  const renderTreeNode = (entry: FileEntry, depth: number) => {
    if (entry.is_dir) {
      const isExpanded = expandedDirs.has(entry.path);
      const isLoading = loadingDirs.has(entry.path);
      const children = dirChildren[entry.path] || [];
      return (
        <div key={entry.path}>
          <button
            className="explorer-tree-entry explorer-tree-dir"
            style={{ paddingLeft: `${8 + depth * 14}px` }}
            onClick={() => toggleDir(entry.path)}
          >
            {isLoading ? (
              <Loader2 size={12} className="spin explorer-tree-chevron" />
            ) : isExpanded ? (
              <ChevronDown size={12} className="explorer-tree-chevron" />
            ) : (
              <ChevronRight size={12} className="explorer-tree-chevron" />
            )}
            <Folder size={14} className="file-icon folder-icon" />
            <span className="file-name">{entry.name}</span>
          </button>
          {isExpanded && children.length > 0 && (
            <div className="explorer-tree-children">
              {children.map((child) => renderTreeNode(child, depth + 1))}
            </div>
          )}
        </div>
      );
    }

    const isActive = openFile?.path === entry.path;
    return (
      <button
        key={entry.path}
        className={`explorer-tree-entry explorer-tree-file ${isActive ? "active" : ""}`}
        style={{ paddingLeft: `${8 + depth * 14 + 14}px` }}
        onClick={() => loadFile(entry.path)}
      >
        <File size={14} className="file-icon" />
        <span className="file-name">{entry.name}</span>
      </button>
    );
  };

  // ── Desktop layout: explorer + editor side by side ────

  if (isDesktop) {
    return (
      <div className="code-editor-panel code-editor-desktop" ref={editorRef}>
        {/* File explorer (collapsible) */}
        {!explorerCollapsed && (
          <div className="code-editor-explorer">
            <div className="explorer-header">
              <span className="explorer-title">Explorer</span>
              <button
                className="explorer-collapse-btn"
                onClick={() => setExplorerCollapsed(true)}
                title="Collapse explorer"
                aria-label="Collapse explorer"
              >
                <PanelLeftClose size={14} />
              </button>
            </div>

            {/* Open files list */}
            {openFiles.length > 0 && (
              <div className="explorer-open-files">
                <div className="explorer-section-label">Open Files</div>
                {openFiles.map((f) => {
                  const name = f.path.split("/").pop() || f.path;
                  const isActive = f.path === activeFilePath;
                  return (
                    <div
                      key={f.path}
                      className={`explorer-open-file${isActive ? " active" : ""}`}
                      onClick={() => {
                        setActiveFilePath(f.path);
                        setSaveStatus(f.editedContent !== null ? "modified" : null);
                      }}
                      title={f.path}
                    >
                      <File size={13} className="file-icon" />
                      <span className="file-name">{name}</span>
                      {f.editedContent !== null && (
                        <span className="open-file-modified-dot" />
                      )}
                      <button
                        className="open-file-close"
                        onClick={(e) => {
                          e.stopPropagation();
                          closeFile(f.path);
                        }}
                        aria-label={`Close ${name}`}
                      >
                        <X size={12} />
                      </button>
                    </div>
                  );
                })}
              </div>
            )}

            <div className="explorer-section-label">Files</div>
            <div className="explorer-tree">
              {loadingDir ? (
                <div className="code-editor-loading">
                  <Loader2 size={16} className="spin" />
                </div>
              ) : entries.length === 0 ? (
                <div className="code-editor-empty">Empty directory</div>
              ) : (
                entries.map((entry) => renderTreeNode(entry, 0))
              )}
            </div>
          </div>
        )}

        {/* Editor area */}
        <div className="code-editor-main">
          {explorerCollapsed && (
            <button
              className="explorer-expand-btn"
              onClick={() => setExplorerCollapsed(false)}
              title="Show explorer"
              aria-label="Show explorer"
            >
              <PanelLeftOpen size={14} />
            </button>
          )}
          {editorToolbar}
          {editorBody}
        </div>
      </div>
    );
  }

  // ── Mobile layout: navigate between browser & editor ──

  // File is open on mobile — show editor
  if (openFile) {
    return (
      <div className="code-editor-panel" ref={editorRef}>
        {editorToolbar}
        {editorBody}
      </div>
    );
  }

  // No file open on mobile — show file browser
  return (
    <div className="code-editor-panel">
      <div className="code-editor-toolbar">
        <div className="code-editor-breadcrumbs">
          {breadcrumbs.map((crumb, i) => (
            <span key={crumb.path}>
              {i > 0 && <span className="breadcrumb-sep">/</span>}
              <button
                className="breadcrumb-link"
                onClick={() => loadDirectory(crumb.path)}
              >
                {crumb.label}
              </button>
            </span>
          ))}
        </div>
      </div>
      <div className="code-editor-filelist">
        {loadingDir ? (
          <div className="code-editor-loading">
            <Loader2 size={20} className="spin" />
            <span>Loading...</span>
          </div>
        ) : entries.length === 0 ? (
          <div className="code-editor-empty">Empty directory</div>
        ) : (
          entries.map((entry) => (
            <button
              key={entry.path}
              className="code-editor-file-entry"
              onClick={() => handleEntryClick(entry)}
            >
              {entry.is_dir ? (
                <Folder size={14} className="file-icon folder-icon" />
              ) : (
                <File size={14} className="file-icon" />
              )}
              <span className="file-name">{entry.name}</span>
              {!entry.is_dir && (
                <span className="file-size">{formatSize(entry.size)}</span>
              )}
            </button>
          ))
        )}
      </div>
    </div>
  );
}

// ── Helpers ──────────────────────────────────────────────

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// ── CSV Viewer ──────────────────────────────────────────

function CsvViewer({ content }: { content: string }) {
  const rows = useMemo(() => {
    if (!content.trim()) return [];
    return content.split("\n").map((line) => {
      // Simple CSV parsing — handles quoted fields with commas
      const cells: string[] = [];
      let current = "";
      let inQuotes = false;
      for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (ch === '"') {
          if (inQuotes && line[i + 1] === '"') {
            current += '"';
            i++;
          } else {
            inQuotes = !inQuotes;
          }
        } else if (ch === "," && !inQuotes) {
          cells.push(current.trim());
          current = "";
        } else {
          current += ch;
        }
      }
      cells.push(current.trim());
      return cells;
    });
  }, [content]);

  if (rows.length === 0) {
    return (
      <div className="file-preview file-preview-binary">
        <span>Empty CSV file</span>
      </div>
    );
  }

  const header = rows[0];
  const body = rows.slice(1).filter((r) => r.some((c) => c.length > 0));

  return (
    <div className="csv-viewer">
      <table>
        <thead>
          <tr>
            {header.map((cell, i) => (
              <th key={i}>{cell}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {body.map((row, ri) => (
            <tr key={ri}>
              {row.map((cell, ci) => (
                <td key={ci}>{cell}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ── Markdown Viewer ─────────────────────────────────────

function MarkdownViewer({ content }: { content: string }) {
  return (
    <div className="markdown-viewer">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
    </div>
  );
}

function HtmlViewer({ content }: { content: string }) {
  const sanitized = useMemo(() => DOMPurify.sanitize(content), [content]);
  return <iframe className="html-viewer-frame" sandbox="allow-scripts allow-same-origin" srcDoc={sanitized} title="HTML preview" />;
}

function SvgViewer({ content }: { content: string }) {
  const sanitized = useMemo(
    () => DOMPurify.sanitize(content, { USE_PROFILES: { svg: true, svgFilters: true } }),
    [content]
  );
  return <div className="svg-viewer" dangerouslySetInnerHTML={{ __html: sanitized }} />;
}

function MermaidViewer({ content }: { content: string }) {
  const [svg, setSvg] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const id = `mermaid-${Math.random().toString(36).slice(2, 10)}`;
    mermaid.initialize({ startOnLoad: false, theme: "base", securityLevel: "strict" });
    mermaid
      .render(id, content)
      .then((result) => {
        if (!active) return;
        setSvg(result.svg);
        setError(null);
      })
      .catch((err) => {
        if (!active) return;
        setError(err instanceof Error ? err.message : "Failed to render Mermaid diagram");
        setSvg("");
      });
    return () => {
      active = false;
    };
  }, [content]);

  if (error) {
    return <div className="file-preview file-preview-binary">{error}</div>;
  }

  return <div className="mermaid-viewer" dangerouslySetInnerHTML={{ __html: svg }} />;
}
