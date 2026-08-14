/**
 * One branch, local or remote.
 *
 * Rename is an inline field rather than a dialog: it is a reversible edit of
 * the thing already under the pointer, and a modal for it would cost more
 * attention than the operation is worth. Delete is the opposite, so it leaves
 * this file entirely and goes through the shared confirm dialog.
 */

import { useState } from "react";
import { ArrowDown, ArrowUp, Check, GitMerge, Pencil, Trash2 } from "lucide-react";

import { relativeTime } from "./gitFormat";
import type { GitBranch } from "../types";

export interface BranchRowProps {
  branch: GitBranch;
  /** True while any mutation is in flight; every control goes inert. */
  busy: boolean;
  onCheckout: () => void;
  onMerge: () => void;
  onRename: (to: string) => void;
  onDelete: () => void;
}

export function BranchRow({ branch, busy, onCheckout, onMerge, onRename, onDelete }: BranchRowProps) {
  const [renaming, setRenaming] = useState(false);
  const [draft, setDraft] = useState(branch.name);

  const heldElsewhere = Boolean(branch.worktree);
  const checkoutTitle = heldElsewhere
    ? `Checked out in the worktree "${branch.worktree}". A branch cannot be checked out in two worktrees at once.`
    : `Check out ${branch.name}`;

  const commitRename = () => {
    const to = draft.trim();
    setRenaming(false);
    if (to && to !== branch.name) onRename(to);
  };

  return (
    <li className="gitp-branch-row" data-current={branch.current ? "" : undefined}>
      <div className="gitp-branch-main">
        <div className="gitp-branch-line">
          {branch.current ? (
            <Check size={13} className="gitp-branch-current" aria-label="Current branch" />
          ) : null}

          {renaming ? (
            <input
              className="gitp-input gitp-branch-rename"
              value={draft}
              autoFocus
              aria-label={`New name for ${branch.name}`}
              onChange={(event) => setDraft(event.target.value)}
              onBlur={commitRename}
              onKeyDown={(event) => {
                if (event.key === "Enter") commitRename();
                if (event.key === "Escape") {
                  setDraft(branch.name);
                  setRenaming(false);
                }
              }}
            />
          ) : (
            <span className="gitp-branch-name gitp-mono">{branch.name}</span>
          )}

          {branch.ahead > 0 || branch.behind > 0 ? (
            <span
              className="gitp-branch-track"
              title={`${branch.ahead} ahead, ${branch.behind} behind${branch.upstream ? ` ${branch.upstream}` : ""}`}
            >
              {branch.ahead > 0 ? (
                <span className="gitp-branch-ahead">
                  <ArrowUp size={11} aria-hidden="true" />
                  {branch.ahead}
                </span>
              ) : null}
              {branch.behind > 0 ? (
                <span className="gitp-branch-behind">
                  <ArrowDown size={11} aria-hidden="true" />
                  {branch.behind}
                </span>
              ) : null}
            </span>
          ) : null}

          {branch.upstream ? <span className="gitp-branch-upstream gitp-mono">{branch.upstream}</span> : null}

          {branch.worktree ? (
            <span className="gitp-badge gitp-badge-worktree" title={checkoutTitle}>
              {branch.worktree}
            </span>
          ) : null}
        </div>

        <div className="gitp-branch-sub">
          <span className="gitp-branch-subject">{branch.subject}</span>
          <span className="gitp-branch-age">{relativeTime(branch.date)}</span>
        </div>
      </div>

      <div className="gitp-row-actions">
        <button
          type="button"
          className="gitp-btn gitp-btn-quiet"
          disabled={busy || branch.current || heldElsewhere}
          title={checkoutTitle}
          onClick={onCheckout}
        >
          Check out
        </button>
        <button
          type="button"
          className="gitp-icon-btn"
          disabled={busy || branch.current}
          aria-label={`Merge ${branch.name} into the current branch`}
          title={`Merge ${branch.name} into the current branch`}
          onClick={onMerge}
        >
          <GitMerge size={14} aria-hidden="true" />
        </button>
        <button
          type="button"
          className="gitp-icon-btn"
          disabled={busy || branch.remote}
          aria-label={`Rename ${branch.name}`}
          title={branch.remote ? "Remote branches cannot be renamed from here" : `Rename ${branch.name}`}
          onClick={() => {
            setDraft(branch.name);
            setRenaming(true);
          }}
        >
          <Pencil size={14} aria-hidden="true" />
        </button>
        <button
          type="button"
          className="gitp-icon-btn gitp-icon-btn-danger"
          disabled={busy || branch.current}
          aria-label={`Delete ${branch.name}`}
          title={branch.current ? "The current branch cannot be deleted" : `Delete ${branch.name}`}
          onClick={onDelete}
        >
          <Trash2 size={14} aria-hidden="true" />
        </button>
      </div>
    </li>
  );
}
