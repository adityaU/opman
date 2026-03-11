import React, { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { fetchFileEdits, writeFile } from "../api";
import type { FileEditEntry, FileEditsResponse } from "../api";
import type { DiffReviewPanelProps, FileStatus } from "./types";
import { buildDiffStyles } from "./helpers";
import {
  DiffHeader,
  LoadingMessage,
  ErrorMessage,
  EmptyMessage,
  FileListSidebar,
  DiffArea,
  DiffFooter,
} from "./components";

export function DiffReviewPanel({
  onClose,
  sessionId,
  fileEditCount,
}: DiffReviewPanelProps) {
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

  // Accept: keep the new content (already on disk, just mark as accepted)
  const handleAccept = useCallback(async (path: string) => {
    setActionInProgress(path);
    try {
      setAcceptedFiles((prev) => new Set([...prev, path]));
      setRevertedFiles((prev) => {
        const next = new Set(prev);
        next.delete(path);
        return next;
      });
    } finally {
      setActionInProgress(null);
    }
  }, []);

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
        // silently ignore
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
    (path: string): FileStatus => {
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
        <DiffHeader
          editCount={edits.length}
          splitView={splitView}
          onToggleSplitView={() => setSplitView((v) => !v)}
          onRefresh={loadEdits}
          onClose={onClose}
        />

        <div className="diff-review-body">
          {loading && <LoadingMessage />}
          {error && <ErrorMessage message={error} />}
          {!loading && !error && edits.length === 0 && <EmptyMessage />}

          {!loading && !error && edits.length > 0 && (
            <div className="diff-review-content">
              <FileListSidebar
                edits={edits}
                selectedFile={selectedFile}
                pendingCount={pendingCount}
                fileStatus={fileStatus}
                onSelectFile={setSelectedFile}
              />
              <DiffArea
                selectedEdit={selectedEdit}
                splitView={splitView}
                diffStyles={diffStyles}
                actionInProgress={actionInProgress}
                acceptedFiles={acceptedFiles}
                revertedFiles={revertedFiles}
                onAccept={handleAccept}
                onRevert={handleRevert}
              />
            </div>
          )}
        </div>

        {edits.length > 0 && (
          <DiffFooter
            actionInProgress={actionInProgress}
            pendingCount={pendingCount}
            onAcceptAll={handleAcceptAll}
            onRevertAll={handleRevertAll}
          />
        )}
      </div>
    </div>
  );
}
