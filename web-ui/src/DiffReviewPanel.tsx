import React, { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { fetchFileEdits, writeFile } from "./api";
import type { FileEditEntry, FileEditsResponse } from "./api";
import ReactDiffViewer, { DiffMethod } from "react-diff-viewer-continued";
import {
  X,
  Loader2,
  RefreshCw,
  FileCode,
  Check,
  Undo2,
  CheckCheck,
  ChevronDown,
  ChevronRight,
  Columns,
  Rows,
} from "lucide-react";
import { withAlpha } from "./utils/theme";

interface Props {
  onClose: () => void;
  sessionId: string | null;
  /** SSE-driven counter — when it changes, auto-refresh edits */
  fileEditCount: number;
}

/** Extract the filename from a full path */
function basename(path: string): string {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

/** Extract directory from a full path */
function dirname(path: string): string {
  const idx = path.lastIndexOf("/");
  return idx > 0 ? path.substring(0, idx) : "";
}

function buildDiffStyles() {
  const css = getComputedStyle(document.documentElement);
  const success = css.getPropertyValue("--color-success").trim() || "#7fd88f";
  const error = css.getPropertyValue("--color-error").trim() || "#e06c75";
  return {
    variables: {
      dark: {
        diffViewerBackground: "var(--theme-surface-1, var(--color-bg))",
        diffViewerColor: "var(--color-text)",
        addedBackground: withAlpha(success, 0.1),
        addedColor: "var(--color-success)",
        removedBackground: withAlpha(error, 0.1),
        removedColor: "var(--color-error)",
        wordAddedBackground: withAlpha(success, 0.22),
        wordRemovedBackground: withAlpha(error, 0.22),
        addedGutterBackground: withAlpha(success, 0.14),
        removedGutterBackground: withAlpha(error, 0.14),
        gutterBackground: "var(--theme-surface-2, var(--color-bg-element))",
        gutterColor: "var(--color-text-muted)",
        gutterBackgroundDark: "var(--theme-surface-1, var(--color-bg))",
        highlightBackground: "var(--theme-surface-hover, var(--color-surface-hover))",
        highlightGutterBackground: "var(--theme-surface-3, var(--color-bg-element))",
        codeFoldGutterBackground: "var(--theme-surface-2, var(--color-bg-element))",
        codeFoldBackground: "var(--theme-surface-2, var(--color-bg-element))",
        emptyLineBackground: "var(--theme-surface-1, var(--color-bg))",
        codeFoldContentColor: "var(--color-text-muted)",
      },
    },
    line: {
      fontFamily: "var(--font-mono)",
      fontSize: "12px",
    },
    gutter: {
      minWidth: "36px",
    },
    contentText: {
      fontFamily: "var(--font-mono)",
      fontSize: "12px",
      lineHeight: "1.5",
    },
  };
}

export function DiffReviewPanel({
  onClose,
  sessionId,
  fileEditCount,
}: Props) {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [edits, setEdits] = useState<FileEditEntry[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [splitView, setSplitView] = useState(true);
  const [acceptedFiles, setAcceptedFiles] = useState<Set<string>>(new Set());
  const [revertedFiles, setRevertedFiles] = useState<Set<string>>(new Set());
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);
  const diffStyles = useMemo(() => buildDiffStyles(), []);

  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef);
  useEscape(onClose);

  // Fetch file edits
  const loadEdits = useCallback(async () => {
    if (!sessionId) {
      setEdits([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const resp: FileEditsResponse = await fetchFileEdits(sessionId);
      setEdits(resp.edits);
      // Auto-select first file if none selected
      if (resp.edits.length > 0 && !selectedFile) {
        setSelectedFile(resp.edits[0].path);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load file edits");
    } finally {
      setLoading(false);
    }
  }, [sessionId, selectedFile]);

  // Load on mount and when fileEditCount changes
  useEffect(() => {
    loadEdits();
  }, [sessionId, fileEditCount]); // eslint-disable-line react-hooks/exhaustive-deps

  // Currently selected edit
  const selectedEdit = useMemo(
    () => edits.find((e) => e.path === selectedFile) ?? null,
    [edits, selectedFile],
  );

  // Accept: keep the new content (it's already on disk, just mark as accepted)
  const handleAccept = useCallback(
    async (path: string) => {
      setActionInProgress(path);
      try {
        // File is already on disk with new_content; just mark accepted
        setAcceptedFiles((prev) => new Set([...prev, path]));
        setRevertedFiles((prev) => {
          const next = new Set(prev);
          next.delete(path);
          return next;
        });
      } finally {
        setActionInProgress(null);
      }
    },
    [],
  );

  // Revert: write original content back to disk
  const handleRevert = useCallback(
    async (path: string) => {
      const edit = edits.find((e) => e.path === path);
      if (!edit) return;
      setActionInProgress(path);
      try {
        await writeFile(path, edit.original_content);
        setRevertedFiles((prev) => new Set([...prev, path]));
        setAcceptedFiles((prev) => {
          const next = new Set(prev);
          next.delete(path);
          return next;
        });
      } catch {
        // silently ignore — toast could be added here
      } finally {
        setActionInProgress(null);
      }
    },
    [edits],
  );

  // Accept All
  const handleAcceptAll = useCallback(() => {
    const paths = edits.map((e) => e.path);
    setAcceptedFiles(new Set(paths));
    setRevertedFiles(new Set());
  }, [edits]);

  // Revert All
  const handleRevertAll = useCallback(async () => {
    setActionInProgress("__all__");
    try {
      for (const edit of edits) {
        await writeFile(edit.path, edit.original_content);
      }
      setRevertedFiles(new Set(edits.map((e) => e.path)));
      setAcceptedFiles(new Set());
    } catch {
      // partial revert is possible
    } finally {
      setActionInProgress(null);
    }
  }, [edits]);

  /** Get file status badge */
  const fileStatus = useCallback(
    (path: string): "accepted" | "reverted" | null => {
      if (acceptedFiles.has(path)) return "accepted";
      if (revertedFiles.has(path)) return "reverted";
      return null;
    },
    [acceptedFiles, revertedFiles],
  );

  const pendingCount = edits.filter(
    (e) => !acceptedFiles.has(e.path) && !revertedFiles.has(e.path),
  ).length;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        ref={modalRef}
        className="diff-review-modal"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="diff-review-header">
          <FileCode size={16} />
          <span className="diff-review-title">
            Diff Review
            {edits.length > 0 && (
              <span className="diff-review-badge">{edits.length}</span>
            )}
          </span>
          <div className="diff-review-header-actions">
            {/* View toggle */}
            <button
              className="diff-review-view-toggle"
              onClick={() => setSplitView((v) => !v)}
              title={splitView ? "Unified view" : "Split view"}
            >
              {splitView ? <Rows size={14} /> : <Columns size={14} />}
            </button>
            <button
              className="diff-review-refresh"
              onClick={loadEdits}
              title="Refresh"
            >
              <RefreshCw size={14} />
            </button>
            <button
              className="diff-review-close"
              onClick={onClose}
              title="Close (Esc)"
            >
              <X size={14} />
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="diff-review-body">
          {loading && (
            <div className="diff-review-loading">
              <Loader2 size={16} className="spin" />
              <span>Loading file edits...</span>
            </div>
          )}

          {error && (
            <div className="diff-review-error">{error}</div>
          )}

          {!loading && !error && edits.length === 0 && (
            <div className="diff-review-empty">
              <FileCode size={32} strokeWidth={1} />
              <span>No file edits in this session yet.</span>
              <span className="diff-review-empty-hint">
                File changes made by the AI will appear here for review.
              </span>
            </div>
          )}

          {!loading && !error && edits.length > 0 && (
            <div className="diff-review-content">
              {/* File list sidebar */}
              <div className="diff-review-file-list">
                <div className="diff-review-file-list-header">
                  <span>Files ({edits.length})</span>
                  {pendingCount > 0 && (
                    <span className="diff-review-pending-badge">
                      {pendingCount} pending
                    </span>
                  )}
                </div>
                {edits.map((edit) => {
                  const status = fileStatus(edit.path);
                  return (
                    <button
                      key={edit.path}
                      className={`diff-review-file-item ${selectedFile === edit.path ? "selected" : ""} ${status ? `file-${status}` : ""}`}
                      onClick={() => setSelectedFile(edit.path)}
                    >
                      <span className="diff-review-file-indicator">
                        {selectedFile === edit.path ? (
                          <ChevronDown size={12} />
                        ) : (
                          <ChevronRight size={12} />
                        )}
                      </span>
                      <span className="diff-review-file-name">
                        {basename(edit.path)}
                      </span>
                      {status === "accepted" && (
                        <span className="diff-review-file-status accepted">
                          <Check size={10} />
                        </span>
                      )}
                      {status === "reverted" && (
                        <span className="diff-review-file-status reverted">
                          <Undo2 size={10} />
                        </span>
                      )}
                      <span className="diff-review-file-dir">
                        {dirname(edit.path)}
                      </span>
                    </button>
                  );
                })}
              </div>

              {/* Diff viewer */}
              <div className="diff-review-diff-area">
                {selectedEdit ? (
                  <>
                    {/* File action bar */}
                    <div className="diff-review-file-actions">
                      <span className="diff-review-file-path">
                        {selectedEdit.path}
                      </span>
                      <div className="diff-review-file-btns">
                        <button
                          className="diff-review-btn accept"
                          onClick={() => handleAccept(selectedEdit.path)}
                          disabled={
                            actionInProgress !== null ||
                            acceptedFiles.has(selectedEdit.path)
                          }
                          title="Accept changes (keep new version)"
                        >
                          <Check size={12} />
                          Accept
                        </button>
                        <button
                          className="diff-review-btn revert"
                          onClick={() => handleRevert(selectedEdit.path)}
                          disabled={
                            actionInProgress !== null ||
                            revertedFiles.has(selectedEdit.path)
                          }
                          title="Revert to original"
                        >
                          <Undo2 size={12} />
                          Revert
                        </button>
                      </div>
                    </div>
                    {/* Diff content */}
                    <div className="diff-review-viewer">
                      <ReactDiffViewer
                        oldValue={selectedEdit.original_content}
                        newValue={selectedEdit.new_content}
                        splitView={splitView}
                        compareMethod={DiffMethod.WORDS}
                        useDarkTheme={true}
                        styles={diffStyles}
                        leftTitle="Original"
                        rightTitle="Modified"
                      />
                    </div>
                  </>
                ) : (
                  <div className="diff-review-no-selection">
                    Select a file from the list to view changes.
                  </div>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Footer with bulk actions */}
        {edits.length > 0 && (
          <div className="diff-review-footer">
            <div className="diff-review-bulk-actions">
              <button
                className="diff-review-btn accept-all"
                onClick={handleAcceptAll}
                disabled={actionInProgress !== null || pendingCount === 0}
              >
                <CheckCheck size={12} />
                Accept All
              </button>
              <button
                className="diff-review-btn revert-all"
                onClick={handleRevertAll}
                disabled={actionInProgress !== null || pendingCount === 0}
              >
                <Undo2 size={12} />
                Revert All
              </button>
            </div>
            <div className="diff-review-footer-hint">
              <kbd>Esc</kbd> to close
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
