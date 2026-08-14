/**
 * The commit list, and one commit at a time in place of it.
 *
 * Rows carry no ref chips: the log endpoint does not report which commits are
 * branch tips, and matching a branch's subject and date to a commit guesses
 * wrong on rebases and duplicated subjects. A missing chip is a small loss; a
 * wrong one sends someone to reset the wrong branch.
 */

import { useCallback, useState } from "react";

import type { GitActionRunner } from "../state/useGitAction";
import type { GitData } from "../state/useGitData";
import { relativeTime, subjectOf } from "../components/gitFormat";
import { CommitDetail } from "./CommitDetail";

export interface HistoryViewProps {
  data: GitData;
  scope: string;
  action: GitActionRunner;
}

const PAGE = 100;

export function HistoryView({ data, scope, action }: HistoryViewProps) {
  const [selected, setSelected] = useState<{ hash: string; short: string } | null>(null);
  const [limit, setLimit] = useState(PAGE);
  const [loadingMore, setLoadingMore] = useState(false);

  // Stable, so the detail's bridge registration is not torn down and rebuilt on
  // every render of the history view.
  const back = useCallback(() => setSelected(null), []);

  if (selected) {
    return (
      <CommitDetail
        hash={selected.hash}
        shortHash={selected.short}
        scope={scope}
        action={action}
        onBack={back}
      />
    );
  }

  if (data.log.length === 0) {
    return (
      <div className="gitp-empty">
        <p className="gitp-empty-title">No commits yet.</p>
        <p className="gitp-empty-body">
          Once you make your first commit in Changes, every commit on this branch will be listed here — newest first —
          with its diff, and with revert, cherry-pick and reset available on each one.
        </p>
      </div>
    );
  }

  const loadMore = () => {
    const next = limit + PAGE;
    setLimit(next);
    setLoadingMore(true);
    void data
      .refreshLog(next)
      .catch((cause: unknown) => console.error("git log failed", cause))
      .finally(() => setLoadingMore(false));
  };

  return (
    <div className="gitp-history">
      <ul className="gitp-commit-list">
        {data.log.map((commit) => (
          <li key={commit.hash}>
            <button
              type="button"
              className="gitp-commit-row"
              onClick={() => setSelected({ hash: commit.hash, short: commit.short_hash })}
            >
              <span className="gitp-mono gitp-commit-hash">{commit.short_hash}</span>
              <span className="gitp-commit-subject">{subjectOf(commit.message)}</span>
              <span className="gitp-commit-author">{commit.author}</span>
              <span className="gitp-commit-date" title={commit.date}>
                {relativeTime(commit.date)}
              </span>
            </button>
          </li>
        ))}
      </ul>

      {data.log.length >= limit ? (
        <div className="gitp-load-more">
          <button type="button" className="gitp-btn gitp-btn-quiet" disabled={loadingMore} onClick={loadMore}>
            {loadingMore ? "Loading…" : "Load more"}
          </button>
        </div>
      ) : (
        <p className="gitp-list-end">That is the whole history of this branch.</p>
      )}
    </div>
  );
}
