/**
 * One commit, opened in place of the list.
 *
 * The three actions here all rewrite history in some way, so they are stated
 * in plain terms — revert makes a new commit, cherry-pick copies this one,
 * reset moves the branch — and the two that can destroy work are confirmed.
 */

import { useEffect, useMemo, useState } from "react";
import { ArrowLeft, CopyPlus, RotateCcw, Undo2 } from "lucide-react";

import * as api from "../api";
import { useGitPanelBridge } from "../state/GitPanelContext";
import type { GitViewHandlers } from "../state/GitPanelContext";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { DiffView } from "../components/DiffView";
import { relativeTime, splitPath, statusLabel, subjectOf } from "../components/gitFormat";
import type { GitActionRunner } from "../state/useGitAction";
import type { GitResetMode, GitShow } from "../types";

export interface CommitDetailProps {
  hash: string;
  shortHash: string;
  scope: string;
  action: GitActionRunner;
  onBack: () => void;
}

type Confirming = { kind: "revert" } | { kind: "reset"; mode: GitResetMode } | null;

const RESET_MODES: Array<{ mode: GitResetMode; label: string; hint: string }> = [
  { mode: "soft", label: "Soft", hint: "Move the branch; keep everything staged." },
  { mode: "mixed", label: "Mixed", hint: "Move the branch; keep the files, unstaged." },
  { mode: "hard", label: "Hard", hint: "Move the branch and throw away all changes." },
];

export function CommitDetail({ hash, shortHash, scope, action, onBack }: CommitDetailProps) {
  const [show, setShow] = useState<GitShow | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<Confirming>(null);
  const busy = action.pending !== null;

  useEffect(() => {
    let live = true;
    setShow(null);
    setError(null);
    api
      .fetchShow(scope, hash)
      .then((result) => live && setShow(result))
      .catch((cause: unknown) => {
        console.error("git show failed", cause);
        if (live) setError(cause instanceof Error ? cause.message : String(cause));
      });
    return () => {
      live = false;
    };
  }, [scope, hash]);

  // Registered only while a detail is on screen, which is exactly when Back has
  // somewhere to go: this component is mounted in place of the commit list.
  const bridge = useGitPanelBridge();
  const handlers = useMemo<GitViewHandlers>(() => ({ goBack: onBack }), [onBack]);
  useEffect(() => bridge?.register(handlers), [bridge, handlers]);

  const runReset = (mode: GitResetMode) =>
    void action.run(`reset ${mode}`, () => api.reset(scope, hash, mode));

  const body = show?.message ?? "";
  const subject = subjectOf(body);
  const rest = body.slice(subject.length).trim();

  return (
    <div className="gitp-commit-detail">
      <div className="gitp-detail-head">
        <button type="button" className="gitp-btn gitp-btn-quiet" onClick={onBack}>
          <ArrowLeft size={14} aria-hidden="true" />
          All commits
        </button>
        <span className="gitp-mono gitp-detail-hash">{shortHash}</span>
      </div>

      {error ? (
        <p className="gitp-detail-error" role="alert">
          This commit could not be read: {error}
        </p>
      ) : null}

      {show ? (
        <>
          <h3 className="gitp-detail-subject">{subject}</h3>
          {rest ? <pre className="gitp-detail-body">{rest}</pre> : null}
          <p className="gitp-detail-meta">
            <span className="gitp-detail-author">{show.author}</span>
            <span className="gitp-detail-date" title={show.date}>
              {relativeTime(show.date)}
            </span>
          </p>

          <div className="gitp-detail-actions">
            <button
              type="button"
              className="gitp-btn"
              disabled={busy}
              onClick={() => setConfirming({ kind: "revert" })}
            >
              <Undo2 size={14} aria-hidden="true" />
              Revert
            </button>
            <button
              type="button"
              className="gitp-btn"
              disabled={busy}
              onClick={() => void action.run("cherry-pick", () => api.cherryPick(scope, hash))}
            >
              <CopyPlus size={14} aria-hidden="true" />
              Cherry-pick
            </button>
            <span className="gitp-reset-group">
              <span className="gitp-reset-label">
                <RotateCcw size={14} aria-hidden="true" />
                Reset to here
              </span>
              {RESET_MODES.map((entry) => (
                <button
                  key={entry.mode}
                  type="button"
                  className={`gitp-btn gitp-btn-quiet${entry.mode === "hard" ? " gitp-btn-danger" : ""}`}
                  disabled={busy}
                  title={entry.hint}
                  onClick={() =>
                    entry.mode === "hard" ? setConfirming({ kind: "reset", mode: "hard" }) : runReset(entry.mode)
                  }
                >
                  {entry.label}
                </button>
              ))}
            </span>
          </div>

          <ul className="gitp-detail-files">
            {show.files.map((file) => {
              const { dir, name } = splitPath(file.path);
              return (
                <li key={file.path} className="gitp-detail-file">
                  <span
                    className="gitp-file-status"
                    data-status={file.status.trim().charAt(0)}
                    title={statusLabel(file.status)}
                  >
                    {file.status.trim().charAt(0) || "?"}
                  </span>
                  <span className="gitp-file-path">
                    {dir ? <span className="gitp-file-dir">{dir}</span> : null}
                    <span className="gitp-file-name">{name}</span>
                  </span>
                </li>
              );
            })}
          </ul>

          <DiffView diff={show.diff} emptyLabel="This commit changed no file contents." />
        </>
      ) : (
        !error && (
          <p className="gitp-detail-loading" aria-live="polite">
            Loading commit…
          </p>
        )
      )}

      <ConfirmDialog
        open={confirming !== null}
        title={confirming?.kind === "revert" ? "Revert this commit?" : "Reset hard to this commit?"}
        body={
          confirming?.kind === "revert" ? (
            <p>
              A new commit will be added that undoes {shortHash}. History is kept; the change is reversed on top.
            </p>
          ) : (
            <>
              <p>The current branch will move to {shortHash}.</p>
              <p>Every commit after it, and every uncommitted change, is thrown away.</p>
            </>
          )
        }
        confirmLabel={confirming?.kind === "revert" ? "Revert" : "Reset hard"}
        danger
        requireTyped={confirming?.kind === "reset" ? shortHash : undefined}
        onConfirm={() => {
          const target = confirming;
          setConfirming(null);
          if (target?.kind === "revert") void action.run("revert", () => api.revert(scope, hash));
          else if (target?.kind === "reset") runReset(target.mode);
        }}
        onCancel={() => setConfirming(null)}
      />
    </div>
  );
}
