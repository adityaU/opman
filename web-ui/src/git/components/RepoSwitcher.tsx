/**
 * Scope selector for projects that contain more than one repository.
 *
 * Hidden entirely at one repository: a picker with a single option is noise
 * that costs a row of a 280px-wide panel.
 */

import { forwardRef, useImperativeHandle, useRef, useState } from "react";
import { Check, ChevronDown, FolderGit2 } from "lucide-react";

import { Popover } from "./Popover";
import type { GitReposState } from "../state/useGitRepos";
import type { GitRepoEntry } from "../types";

export interface RepoSwitcherProps {
  repos: GitReposState;
  disabled?: boolean;
}

function changeCount(repo: GitRepoEntry): number {
  return repo.staged_count + repo.unstaged_count + repo.untracked_count;
}

/** Lets the keybinding layer open the switcher, when one is rendered at all. */
export interface RepoSwitcherHandle {
  open: () => void;
}

export const RepoSwitcher = forwardRef<RepoSwitcherHandle, RepoSwitcherProps>(
  function RepoSwitcher({ repos, disabled }: RepoSwitcherProps, ref) {
  const trigger = useRef<HTMLButtonElement | null>(null);
  const [open, setOpen] = useState(false);

  useImperativeHandle(ref, () => ({ open: () => setOpen(true) }), []);

  if (repos.repos.length <= 1) return null;

  const activeName = repos.active?.name ?? "repository";

  return (
    <>
      <button
        ref={trigger}
        type="button"
        className="gitp-repo-trigger"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={`Repository: ${activeName}. Switch repository`}
        title={repos.active?.path ?? activeName}
        disabled={disabled}
        onClick={() => setOpen((value) => !value)}
      >
        <FolderGit2 className="gitp-icon" aria-hidden="true" />
        <span className="gitp-repo-name">{activeName}</span>
        <ChevronDown className="gitp-icon gitp-icon-caret" aria-hidden="true" />
      </button>

      {open ? (
        <Popover anchor={trigger} label="Switch repository" onClose={() => setOpen(false)}>
          <ul className="gitp-popover-list" role="listbox" aria-label="Repositories">
            {repos.repos.map((repo) => {
              const selected = repo.path === repos.scope;
              const changes = changeCount(repo);
              return (
                <li key={repo.path}>
                  <button
                    type="button"
                    role="option"
                    aria-selected={selected}
                    className="gitp-popover-row"
                    data-selected={selected ? "" : undefined}
                    onClick={() => {
                      repos.setScope(repo.path);
                      setOpen(false);
                    }}
                  >
                    <span className="gitp-popover-check">
                      {selected ? <Check className="gitp-icon" aria-hidden="true" /> : null}
                    </span>
                    <span className="gitp-popover-main">
                      <span className="gitp-popover-title">{repo.name}</span>
                      <span className="gitp-popover-sub">{repo.branch}</span>
                    </span>
                    {changes > 0 ? (
                      <span className="gitp-popover-count" title={`${changes} changed files`}>
                        {changes}
                      </span>
                    ) : null}
                  </button>
                </li>
              );
            })}
          </ul>
        </Popover>
      ) : null}
    </>
  );
});
