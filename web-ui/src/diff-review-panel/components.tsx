import React from "react";
import type { FileEditEntry } from "../api";
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
import type { FileStatus } from "./types";
import { basename, dirname } from "./helpers";

/* ---------- Header ---------- */

interface HeaderProps {
  editCount: number;
  splitView: boolean;
  onToggleSplitView: () => void;
  onRefresh: () => void;
  onClose: () => void;
}

export function DiffHeader({
  editCount,
  splitView,
  onToggleSplitView,
  onRefresh,
  onClose,
}: HeaderProps) {
  return (
    <div className="diff-review-header">
      <FileCode size={16} />
      <span className="diff-review-title">
        Diff Review
        {editCount > 0 && (
          <span className="diff-review-badge">{editCount}</span>
        )}
      </span>
      <div className="diff-review-header-actions">
        <button
          className="diff-review-view-toggle"
          onClick={onToggleSplitView}
          title={splitView ? "Unified view" : "Split view"}
        >
          {splitView ? <Rows size={14} /> : <Columns size={14} />}
        </button>
        <button
          className="diff-review-refresh"
          onClick={onRefresh}
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
  );
}

/* ---------- Status messages ---------- */

export function LoadingMessage() {
  return (
    <div className="diff-review-loading">
      <Loader2 size={16} className="spin" />
      <span>Loading file edits...</span>
    </div>
  );
}

export function ErrorMessage({ message }: { message: string }) {
  return <div className="diff-review-error">{message}</div>;
}

export function EmptyMessage() {
  return (
    <div className="diff-review-empty">
      <FileCode size={32} strokeWidth={1} />
      <span>No file edits in this session yet.</span>
      <span className="diff-review-empty-hint">
        File changes made by the AI will appear here for review.
      </span>
    </div>
  );
}

/* ---------- File list sidebar ---------- */

interface FileListProps {
  edits: FileEditEntry[];
  selectedFile: string | null;
  pendingCount: number;
  fileStatus: (path: string) => FileStatus;
  onSelectFile: (path: string) => void;
}

export function FileListSidebar({
  edits,
  selectedFile,
  pendingCount,
  fileStatus,
  onSelectFile,
}: FileListProps) {
  return (
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
            onClick={() => onSelectFile(edit.path)}
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
  );
}

/* ---------- Diff area ---------- */

interface DiffAreaProps {
  selectedEdit: FileEditEntry | null;
  splitView: boolean;
  diffStyles: ReturnType<typeof import("./helpers").buildDiffStyles>;
  actionInProgress: string | null;
  acceptedFiles: Set<string>;
  revertedFiles: Set<string>;
  onAccept: (path: string) => void;
  onRevert: (path: string) => void;
}

export function DiffArea({
  selectedEdit,
  splitView,
  diffStyles,
  actionInProgress,
  acceptedFiles,
  revertedFiles,
  onAccept,
  onRevert,
}: DiffAreaProps) {
  if (!selectedEdit) {
    return (
      <div className="diff-review-diff-area">
        <div className="diff-review-no-selection">
          Select a file from the list to view changes.
        </div>
      </div>
    );
  }

  return (
    <div className="diff-review-diff-area">
      {/* File action bar */}
      <div className="diff-review-file-actions">
        <span className="diff-review-file-path">{selectedEdit.path}</span>
        <div className="diff-review-file-btns">
          <button
            className="diff-review-btn accept"
            onClick={() => onAccept(selectedEdit.path)}
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
            onClick={() => onRevert(selectedEdit.path)}
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
    </div>
  );
}

/* ---------- Footer ---------- */

interface FooterProps {
  actionInProgress: string | null;
  pendingCount: number;
  onAcceptAll: () => void;
  onRevertAll: () => void;
}

export function DiffFooter({
  actionInProgress,
  pendingCount,
  onAcceptAll,
  onRevertAll,
}: FooterProps) {
  return (
    <div className="diff-review-footer">
      <div className="diff-review-bulk-actions">
        <button
          className="diff-review-btn accept-all"
          onClick={onAcceptAll}
          disabled={actionInProgress !== null || pendingCount === 0}
        >
          <CheckCheck size={12} />
          Accept All
        </button>
        <button
          className="diff-review-btn revert-all"
          onClick={onRevertAll}
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
  );
}
