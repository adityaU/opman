import ReactDiffViewer, { DiffMethod } from "react-diff-viewer-continued";
import { Loader2, ChevronDown, ChevronRight } from "lucide-react";
import type { GitShowResponse } from "../types";
import { statusColor, statusLabel, formatRelativeTime, splitDiffByFile, parseUnifiedDiff } from "../utils";

interface Props {
  commitDetail: GitShowResponse | null;
  commitDetailLoading: boolean;
  expandedFiles: Set<string>;
  toggleFileAccordion: (path: string) => void;
  expandAllFiles: () => void;
  collapseAllFiles: () => void;
  diffStyles: ReturnType<typeof import("../utils").buildDiffStyles>;
}

export function CommitDetailView({
  commitDetail, commitDetailLoading,
  expandedFiles, toggleFileAccordion, expandAllFiles, collapseAllFiles,
  diffStyles,
}: Props) {
  if (commitDetailLoading) {
    return <div className="git-loading"><Loader2 size={18} className="spin" /></div>;
  }

  if (!commitDetail) {
    return <div className="git-empty"><span>Failed to load commit</span></div>;
  }

  const perFileDiffs = splitDiffByFile(commitDetail.diff || "");

  return (
    <>
      {/* Commit metadata */}
      <div className="git-commit-meta">
        <div className="git-commit-meta-hash">{commitDetail.hash.slice(0, 10)}</div>
        <div className="git-commit-meta-message">{commitDetail.message}</div>
        <div className="git-commit-meta-info">
          <span>{commitDetail.author}</span>
          <span>{formatRelativeTime(commitDetail.date)}</span>
        </div>
        {commitDetail.files.length > 0 && (
          <div className="git-commit-files-summary">
            <span>{commitDetail.files.length} file{commitDetail.files.length !== 1 ? "s" : ""} changed</span>
            <div className="git-commit-expand-controls">
              <button className="git-commit-expand-btn" onClick={expandAllFiles} title="Expand all files">
                Expand all
              </button>
              <button className="git-commit-expand-btn" onClick={collapseAllFiles} title="Collapse all files">
                Collapse all
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Per-file diff accordions */}
      <div className="git-commit-file-accordions">
        {commitDetail.files.map((f) => {
          const isExpanded = expandedFiles.has(f.path);
          const fileDiff = perFileDiffs.get(f.path) || "";
          const fileName = f.path.split("/").pop() || f.path;
          const dirPath = f.path.includes("/") ? f.path.slice(0, f.path.lastIndexOf("/") + 1) : "";

          return (
            <div key={f.path} className={`git-commit-file-accordion ${isExpanded ? "expanded" : ""}`}>
              <button className="git-commit-file-header" onClick={() => toggleFileAccordion(f.path)}>
                {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                <span className="git-file-status" style={{ color: statusColor(f.status) }} title={statusLabel(f.status)}>
                  {f.status}
                </span>
                <span className="git-commit-file-path">
                  {dirPath && <span className="git-commit-file-dir">{dirPath}</span>}
                  <span className="git-commit-file-name">{fileName}</span>
                </span>
              </button>
              {isExpanded && (
                <div className="git-commit-file-body">
                  {fileDiff ? (
                    <ReactDiffViewer
                      oldValue={parseUnifiedDiff(fileDiff).oldText}
                      newValue={parseUnifiedDiff(fileDiff).newText}
                      splitView={false} useDarkTheme={true}
                      compareMethod={DiffMethod.LINES} styles={diffStyles}
                    />
                  ) : (
                    <div className="git-empty git-empty-sm"><span>No diff available for this file</span></div>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </>
  );
}
