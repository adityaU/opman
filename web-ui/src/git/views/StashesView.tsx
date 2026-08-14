/**
 * Stashes: work set aside, and the three different things "restore" can mean.
 *
 * Apply and Pop are separated rather than folded into one button because the
 * difference — whether the entry survives — is exactly what people get wrong,
 * and it is not recoverable once the entry is gone.
 */

import { useState } from "react";
import { Archive, Trash2 } from "lucide-react";

import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { relativeTime } from "../components/gitFormat";
import type { GitData } from "../state/useGitData";
import type { GitActionRunner } from "../state/useGitAction";
import type { GitStashEntry } from "../types";

export interface StashesViewProps {
  data: GitData;
  scope: string;
  action: GitActionRunner;
}

export function StashesView({ data, scope, action }: StashesViewProps) {
  const [message, setMessage] = useState("");
  const [target, setTarget] = useState<GitStashEntry | null>(null);

  const busy = action.pending !== null;
  const status = data.status;
  const dirty = Boolean(
    status && (status.staged.length || status.unstaged.length || status.untracked.length),
  );

  const push = async () => {
    const result = await action.run("stash changes", () =>
      api.stash(scope, "push", { message: message.trim() || undefined }),
    );
    if (result?.ok) setMessage("");
  };

  return (
    <div className="gitp-stashes">
      <form
        className="gitp-create"
        onSubmit={(event) => {
          event.preventDefault();
          if (dirty) void push();
        }}
      >
        <div className="gitp-create-fields">
          <input
            className="gitp-input"
            value={message}
            placeholder="Description (optional)"
            aria-label="Stash description"
            autoComplete="off"
            disabled={busy || !dirty}
            onChange={(event) => setMessage(event.target.value)}
          />
          <button
            type="submit"
            className="gitp-btn gitp-btn-primary"
            disabled={busy || !dirty}
            title={dirty ? "Set the working tree aside" : "Nothing to stash: the working tree is clean"}
          >
            <Archive size={14} aria-hidden="true" />
            Stash current changes
          </button>
        </div>
      </form>

      <section className="gitp-section" aria-label="Stashes">
        <h3 className="gitp-section-title">
          Stashes <span className="gitp-count">{data.stashes.length}</span>
        </h3>

        {data.stashes.length ? (
          <ul className="gitp-list">
            {data.stashes.map((entry) => (
              <li className="gitp-stash-row" key={entry.reference}>
                <div className="gitp-stash-main">
                  <div className="gitp-stash-line">
                    <span className="gitp-stash-ref gitp-mono">{entry.reference}</span>
                    <span className="gitp-stash-message">{entry.message}</span>
                  </div>
                  <div className="gitp-stash-sub">
                    <span className="gitp-stash-hash gitp-mono">{entry.hash.slice(0, 7)}</span>
                    <span className="gitp-stash-age">{relativeTime(entry.age)}</span>
                  </div>
                </div>
                <div className="gitp-row-actions">
                  <button
                    type="button"
                    className="gitp-btn gitp-btn-quiet"
                    disabled={busy}
                    title="Restore these changes and keep the stash entry"
                    onClick={() =>
                      void action.run("apply stash", () =>
                        api.stash(scope, "apply", { stashRef: entry.reference }),
                      )
                    }
                  >
                    Apply
                  </button>
                  <button
                    type="button"
                    className="gitp-btn gitp-btn-quiet"
                    disabled={busy}
                    title="Restore these changes and remove the stash entry"
                    onClick={() =>
                      void action.run("pop stash", () =>
                        api.stash(scope, "pop", { stashRef: entry.reference }),
                      )
                    }
                  >
                    Pop
                  </button>
                  <button
                    type="button"
                    className="gitp-icon-btn gitp-icon-btn-danger"
                    disabled={busy}
                    aria-label={`Drop ${entry.reference}`}
                    title={`Drop ${entry.reference}`}
                    onClick={() => setTarget(entry)}
                  >
                    <Trash2 size={14} aria-hidden="true" />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <div className="gitp-empty gitp-empty-rich">
            <Archive size={20} aria-hidden="true" />
            <p className="gitp-empty-title">No stashes</p>
            <p className="gitp-empty-body">
              Stashing puts your uncommitted changes to one side and gives you a clean working tree, so you
              can switch branches or pull without committing half-finished work.
            </p>
          </div>
        )}
      </section>

      <ConfirmDialog
        open={target !== null}
        title="Drop stash"
        danger
        confirmLabel="Drop stash"
        body={
          <p className="gitp-confirm-text">
            <code className="gitp-mono">{target?.reference}</code> — {target?.message} — will be discarded.
            These changes were never committed, so nothing else refers to them.
          </p>
        }
        onCancel={() => setTarget(null)}
        onConfirm={() => {
          const entry = target;
          setTarget(null);
          if (entry) {
            void action.run("drop stash", () => api.stash(scope, "drop", { stashRef: entry.reference }));
          }
        }}
      />
    </div>
  );
}
