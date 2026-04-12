import { Loader2 } from "lucide-react";
import type { GitLogEntry, GitView } from "../types";
import { formatRelativeTime } from "../utils";

interface Props {
  logLoading: boolean;
  commits: GitLogEntry[];
  pushView: (v: GitView) => void;
}

export function LogListView({ logLoading, commits, pushView }: Props) {
  if (logLoading) {
    return <div className="git-loading"><Loader2 size={18} className="spin" /></div>;
  }

  if (commits.length === 0) {
    return <div className="git-empty"><span>No commits found</span></div>;
  }

  return (
    <>
      {commits.map((c) => (
        <div
          key={c.hash} className="git-log-entry git-log-entry-clickable"
          onClick={() => pushView({ kind: "commit", hash: c.hash, shortHash: c.short_hash })}
          role="button" tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              pushView({ kind: "commit", hash: c.hash, shortHash: c.short_hash });
            }
          }}
        >
          <div className="git-log-hash">{c.short_hash}</div>
          <div className="git-log-message">{c.message}</div>
          <div className="git-log-meta">
            <span>{c.author}</span>
            <span>{formatRelativeTime(c.date)}</span>
          </div>
        </div>
      ))}
    </>
  );
}
