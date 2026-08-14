/**
 * The branch identity control and its switcher.
 *
 * Detachment is rendered as a warning rather than as a branch name on purpose:
 * committing while detached loses work silently, so the one place a person
 * always looks — the branch line — has to say so.
 */

import { forwardRef, useImperativeHandle, useMemo, useRef, useState } from "react";
import { AlertTriangle, Check, ChevronDown, GitBranch, Lock, Search } from "lucide-react";

import * as api from "../api";
import { Popover } from "./Popover";
import type { GitActionRunner } from "../state/useGitAction";
import type { GitBranch as GitBranchEntry, GitBranches } from "../types";

export interface BranchPopoverProps {
  branches: GitBranches | null;
  scope: string;
  action: GitActionRunner;
}

function shortHash(ref: string): string {
  return ref.replace(/^HEAD detached at /, "").slice(0, 8);
}

function matches(branch: GitBranchEntry, filter: string): boolean {
  return !filter || branch.name.toLowerCase().includes(filter.toLowerCase());
}

/** Lets the keybinding layer open the picker the header already owns. */
export interface BranchPopoverHandle {
  open: () => void;
}

export const BranchPopover = forwardRef<BranchPopoverHandle, BranchPopoverProps>(
  function BranchPopover({ branches, scope, action }: BranchPopoverProps, ref) {
  const trigger = useRef<HTMLButtonElement | null>(null);
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");

  const detached = branches?.detached ?? false;
  const current = branches?.current ?? "";
  const busy = action.pending !== null;

  const local = useMemo(
    () => (branches?.local ?? []).filter((branch) => matches(branch, filter)),
    [branches, filter],
  );
  const remote = useMemo(
    () => (branches?.remote ?? []).filter((branch) => matches(branch, filter)),
    [branches, filter],
  );

  useImperativeHandle(ref, () => ({ open: () => setOpen(true) }), []);

  const checkout = (name: string) => {
    setOpen(false);
    void action.run("checkout", () => api.checkout(scope, name));
  };

  const renderRow = (branch: GitBranchEntry) => {
    const held = Boolean(branch.worktree);
    return (
      <li key={`${branch.remote ? "r" : "l"}:${branch.name}`}>
        <button
          type="button"
          role="option"
          aria-selected={branch.current}
          className="gitp-popover-row"
          data-selected={branch.current ? "" : undefined}
          disabled={held || branch.current}
          title={held ? `Checked out in ${branch.worktree}` : branch.subject || branch.name}
          onClick={() => checkout(branch.name)}
        >
          <span className="gitp-popover-check">
            {branch.current ? <Check className="gitp-icon" aria-hidden="true" /> : null}
          </span>
          <span className="gitp-popover-main">
            <span className="gitp-popover-title">{branch.name}</span>
            {branch.subject ? <span className="gitp-popover-sub">{branch.subject}</span> : null}
          </span>
          {held ? (
            <Lock className="gitp-icon gitp-popover-lock" aria-label="Held by another worktree" />
          ) : null}
        </button>
      </li>
    );
  };

  return (
    <>
      <button
        ref={trigger}
        type="button"
        className="gitp-branch-trigger"
        data-detached={detached ? "" : undefined}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={detached ? `Detached at ${shortHash(current)}. Switch branch` : `Branch ${current}. Switch branch`}
        disabled={busy}
        onClick={() => setOpen((value) => !value)}
      >
        {detached ? (
          <AlertTriangle className="gitp-icon gitp-branch-warn" aria-hidden="true" />
        ) : (
          <GitBranch className="gitp-icon" aria-hidden="true" />
        )}
        <span className="gitp-branch-name" data-detached={detached ? "" : undefined}>
          {detached ? `detached at ${shortHash(current)}` : current || "no branch"}
        </span>
        <ChevronDown className="gitp-icon gitp-icon-caret" aria-hidden="true" />
      </button>

      {open ? (
        <Popover anchor={trigger} label="Switch branch" onClose={() => setOpen(false)}>
          <div className="gitp-popover-filter">
            <Search className="gitp-icon" aria-hidden="true" />
            <input
              type="text"
              className="gitp-popover-input"
              placeholder="Filter branches"
              aria-label="Filter branches"
              autoFocus
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
            />
          </div>
          <div className="gitp-popover-scroll">
            {local.length > 0 ? (
              <>
                <p className="gitp-popover-group">Local</p>
                <ul className="gitp-popover-list" role="listbox" aria-label="Local branches">
                  {local.map(renderRow)}
                </ul>
              </>
            ) : null}
            {remote.length > 0 ? (
              <>
                <p className="gitp-popover-group">Remote</p>
                <ul className="gitp-popover-list" role="listbox" aria-label="Remote branches">
                  {remote.map(renderRow)}
                </ul>
              </>
            ) : null}
            {local.length === 0 && remote.length === 0 ? (
              <p className="gitp-popover-empty">No branch matches “{filter}”.</p>
            ) : null}
          </div>
        </Popover>
      ) : null}
    </>
  );
});
