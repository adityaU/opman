import { ChevronDown, ChevronRight, Plus, Minus, Trash2 } from "lucide-react";
import type { GitFileEntry, GitView } from "../types";
import { statusColor, statusLabel } from "../utils";

interface Props {
  title: string;
  files: GitFileEntry[];
  isOpen: boolean;
  onToggle: () => void;
  pushView: (v: GitView) => void;
  /** Section variant determines available actions */
  variant: "staged" | "unstaged" | "untracked";
  /** Bulk action (stage all / unstage all) */
  onBulkAction: () => void;
  /** Per-file primary action (stage or unstage) */
  onFileAction: (files: string[]) => void;
  /** Per-file discard (only for unstaged) */
  onDiscard?: (files: string[]) => void;
}

export function FileSection({
  title, files, isOpen, onToggle, pushView, variant,
  onBulkAction, onFileAction, onDiscard,
}: Props) {
  if (files.length === 0) return null;

  const isStaged    = variant === "staged";
  const isUntracked = variant === "untracked";

  return (
    <div className="git-section">
      <button className="git-section-header" onClick={onToggle}>
        {isOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <span className="git-section-title">{title} ({files.length})</span>
        <button
          className="git-section-action"
          onClick={(e) => { e.stopPropagation(); onBulkAction(); }}
          title={isStaged ? "Unstage all" : "Stage all"}
          aria-label={isStaged ? "Unstage all files" : "Stage all files"}
        >
          {isStaged ? <Minus size={11} /> : <Plus size={11} />}
        </button>
      </button>

      {isOpen && files.map((file) => (
        <div
          key={`${variant}-${file.path}`}
          className="git-file-row"
          onClick={!isUntracked ? () => pushView({ kind: "file-diff", file: file.path, staged: isStaged }) : undefined}
          role={!isUntracked ? "button" : undefined}
          tabIndex={!isUntracked ? 0 : undefined}
          onKeyDown={!isUntracked ? (e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              pushView({ kind: "file-diff", file: file.path, staged: isStaged });
            }
          } : undefined}
        >
          <span
            className="git-file-status"
            style={{ color: statusColor(file.status) }}
            title={statusLabel(file.status)}
          >
            {isUntracked ? "?" : file.status}
          </span>
          <span className="git-file-path">{file.path}</span>

          {variant === "unstaged" ? (
            <div className="git-file-actions">
              <button
                className="git-file-action"
                onClick={(e) => { e.stopPropagation(); onDiscard?.([file.path]); }}
                title="Discard changes"
                aria-label={`Discard changes to ${file.path}`}
              >
                <Trash2 size={11} />
              </button>
              <button
                className="git-file-action"
                onClick={(e) => { e.stopPropagation(); onFileAction([file.path]); }}
                title="Stage" aria-label={`Stage ${file.path}`}
              >
                <Plus size={11} />
              </button>
            </div>
          ) : (
            <button
              className="git-file-action"
              onClick={(e) => { e.stopPropagation(); onFileAction([file.path]); }}
              title={isStaged ? "Unstage" : "Stage"}
              aria-label={`${isStaged ? "Unstage" : "Stage"} ${file.path}`}
            >
              {isStaged ? <Minus size={11} /> : <Plus size={11} />}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
