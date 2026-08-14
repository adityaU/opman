/**
 * The mid-operation banner.
 *
 * When a merge or rebase is in flight the repository is in a state where
 * almost every other control is a mistake, so this outranks everything else on
 * the panel: it names the operation, lists what is blocking it, and offers the
 * only three moves git actually has.
 */

import { CircleAlert, Loader2, Play, SkipForward, Undo2 } from "lucide-react";

import * as api from "../api";
import type { GitActionRunner } from "../state/useGitAction";
import type { GitOperation, GitOperationKind } from "../types";

export interface OperationBannerProps {
  operation: GitOperation;
  scope: string;
  action: GitActionRunner;
}

const NAMES: Record<GitOperationKind, string> = {
  merge: "Merge",
  rebase: "Rebase",
  cherry_pick: "Cherry-pick",
  revert: "Revert",
  bisect: "Bisect",
};

/** Merge has no `--skip`: there is no next commit to move past. */
const SKIPPABLE: GitOperationKind[] = ["rebase", "cherry_pick", "revert"];

export function OperationBanner({ operation, scope, action }: OperationBannerProps) {
  const kind = operation.kind;
  const name = kind ? NAMES[kind] : "Operation";
  const conflicts = operation.conflicted;
  const blocked = conflicts.length > 0;
  const busy = action.pending !== null;
  const canSkip = kind ? SKIPPABLE.includes(kind) : false;

  const resolve = (mode: "continue" | "abort" | "skip") => {
    void action.run(`${kind ?? "operation"}-${mode}`, () => api.resolveOperation(scope, mode));
  };

  const pendingFor = (mode: string) => action.pending === `${kind ?? "operation"}-${mode}`;

  return (
    <section className="gitp-op" role="alert" data-kind={kind ?? "operation"}>
      <div className="gitp-op-head">
        <CircleAlert className="gitp-icon gitp-op-icon" aria-hidden="true" />
        <div className="gitp-op-title">
          <strong className="gitp-op-name">{name} in progress</strong>
          <span className="gitp-op-meta">
            {operation.step !== undefined && operation.total !== undefined
              ? `step ${operation.step} of ${operation.total}`
              : null}
            {operation.onto ? (
              <span className="gitp-op-onto">
                onto <code className="gitp-mono-inline">{operation.onto}</code>
              </span>
            ) : null}
          </span>
        </div>
      </div>

      {blocked ? (
        <div className="gitp-op-conflicts">
          <p className="gitp-op-conflicts-title">
            {conflicts.length} conflicted {conflicts.length === 1 ? "file" : "files"}
          </p>
          <ul className="gitp-op-conflict-list">
            {conflicts.map((path) => (
              <li key={path} className="gitp-op-conflict" title={path}>
                {path}
              </li>
            ))}
          </ul>
        </div>
      ) : (
        <p className="gitp-op-clear">No conflicts remain — continue to finish the {name.toLowerCase()}.</p>
      )}

      <div className="gitp-op-actions">
        <button
          type="button"
          className="gitp-btn gitp-btn-primary"
          disabled={busy || blocked}
          title={
            blocked
              ? "Resolve and stage every conflicted file before continuing"
              : `Continue the ${name.toLowerCase()}`
          }
          onClick={() => resolve("continue")}
        >
          {pendingFor("continue") ? (
            <Loader2 className="gitp-icon gitp-spin" aria-hidden="true" />
          ) : (
            <Play className="gitp-icon" aria-hidden="true" />
          )}
          <span>Continue</span>
        </button>

        {canSkip ? (
          <button
            type="button"
            className="gitp-btn"
            disabled={busy}
            title={`Skip this commit and move on with the ${name.toLowerCase()}`}
            onClick={() => resolve("skip")}
          >
            {pendingFor("skip") ? (
              <Loader2 className="gitp-icon gitp-spin" aria-hidden="true" />
            ) : (
              <SkipForward className="gitp-icon" aria-hidden="true" />
            )}
            <span>Skip this commit</span>
          </button>
        ) : null}

        <button
          type="button"
          className="gitp-btn gitp-btn-danger"
          disabled={busy}
          title={`Stop and return the repository to where it was before this ${name.toLowerCase()}`}
          onClick={() => resolve("abort")}
        >
          {pendingFor("abort") ? (
            <Loader2 className="gitp-icon gitp-spin" aria-hidden="true" />
          ) : (
            <Undo2 className="gitp-icon" aria-hidden="true" />
          )}
          <span>Abort — discard this {name.toLowerCase()}</span>
        </button>
      </div>
    </section>
  );
}
