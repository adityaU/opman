/**
 * One titled group of files — Staged, Changed, Untracked.
 *
 * A group with no files renders nothing at all rather than an empty heading:
 * three permanent headers with zeroes read as clutter, while their appearance
 * and disappearance is itself the signal that something moved.
 */

import type { ReactNode } from "react";

import { FileRow } from "./FileRow";
import type { GitFileEntry } from "../types";
import type { GitSectionVariant, GitSelection } from "../state/useGitSelection";
import { DiffView } from "./DiffView";

export interface FileGroupProps {
  title: string;
  files: GitFileEntry[];
  staged: boolean;
  /**
   * The section these files belong to, passed in rather than inferred: the
   * title is display copy, and "Changed" vs "unstaged" would drift apart.
   */
  variant: GitSectionVariant;
  /** Keyboard cursor, absent when the group is rendered outside the panel. */
  selection?: GitSelection;
  bulkLabel: string;
  onBulk: () => void;
  disabled?: boolean;
  /** Path whose diff is open in this group, if any. */
  openPath: string | null;
  openDiff: string | null;
  diffLoading: boolean;
  onOpenDiff: (path: string, staged: boolean) => void;
  onStage: (path: string) => void;
  onUnstage: (path: string) => void;
  onDiscard?: (path: string) => void;
  /** Extra controls rendered next to the bulk action. */
  extra?: ReactNode;
}

export function FileGroup({
  title,
  files,
  staged,
  variant,
  selection,
  bulkLabel,
  onBulk,
  disabled,
  openPath,
  openDiff,
  diffLoading,
  onOpenDiff,
  onStage,
  onUnstage,
  onDiscard,
  extra,
}: FileGroupProps) {
  if (files.length === 0) return null;

  return (
    <section className="gitp-file-group">
      <header className="gitp-group-head">
        <h3 className="gitp-group-title">
          {title}
          <span className="gitp-group-count">{files.length}</span>
        </h3>
        <span className="gitp-group-actions">
          {extra}
          <button type="button" className="gitp-btn gitp-btn-quiet" disabled={disabled} onClick={onBulk}>
            {bulkLabel}
          </button>
        </span>
      </header>

      <div className="gitp-file-list">
        {files.map((file) => (
          <div key={file.path} className="gitp-file-slot">
            <FileRow
              path={file.path}
              status={file.status}
              staged={staged}
              selected={openPath === file.path}
              selection={selection}
              variant={variant}
              disabled={disabled}
              onOpenDiff={onOpenDiff}
              onStage={onStage}
              onUnstage={onUnstage}
              onDiscard={onDiscard}
            />
            {openPath === file.path ? (
              <div className="gitp-file-diff">
                {diffLoading && openDiff === null ? (
                  <p className="gitp-diff-note" aria-live="polite">
                    Loading diff…
                  </p>
                ) : (
                  <DiffView
                    diff={openDiff ?? ""}
                    emptyLabel={
                      staged
                        ? "No staged changes remain in this file."
                        : "This file is new to git — stage it to see it tracked."
                    }
                  />
                )}
              </div>
            ) : null}
          </div>
        ))}
      </div>
    </section>
  );
}
