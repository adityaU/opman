/**
 * RepoSwitcher — dropdown to switch between multiple git repos within a workspace.
 */
import { useState, useRef, useCallback } from "react";
import { GitFork, ChevronDown, Check, Loader2 } from "lucide-react";
import { useOutsideClick } from "../hooks/useOutsideClick";
import type { GitRepoEntry } from "../types";

interface Props {
  repos: GitRepoEntry[];
  selectedRepo: string | undefined;
  onSelect: (repoPath: string) => void;
  loading?: boolean;
}

export function RepoSwitcher({ repos, selectedRepo, onSelect, loading }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useOutsideClick(ref, open, () => setOpen(false));

  const toggle = useCallback(() => setOpen((v) => !v), []);

  const selectedEntry = repos.find((r) => r.path === selectedRepo);
  const label = selectedEntry?.name || "Select repo";

  return (
    <div className="git-repo-switcher" ref={ref}>
      <button className="git-repo-toggle" onClick={toggle} disabled={loading}>
        <GitFork size={12} />
        <span className="git-repo-label">{loading ? "Scanning..." : label}</span>
        <ChevronDown size={10} className={open ? "rotated" : ""} />
      </button>
      {open && (
        <div className="git-repo-dropdown">
          {repos.map((repo) => {
            const isCurrent = repo.path === selectedRepo;
            const changeCount = repo.staged_count + repo.unstaged_count + repo.untracked_count;
            return (
              <button
                key={repo.path}
                className={`git-repo-item ${isCurrent ? "current" : ""}`}
                onClick={() => { onSelect(repo.path); setOpen(false); }}
              >
                <div className="git-repo-item-main">
                  <span className="git-repo-item-name">{repo.name}</span>
                  <span className="git-repo-item-branch">{repo.branch}</span>
                </div>
                <div className="git-repo-item-meta">
                  {changeCount > 0 && (
                    <span className="git-repo-item-changes">{changeCount}</span>
                  )}
                  {isCurrent && <Check size={12} />}
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
