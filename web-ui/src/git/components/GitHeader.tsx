/**
 * The panel's identity and sync line: which repository, which branch, how far
 * from the remote, and the three commands that change that distance.
 *
 * Fetch/Pull/Push are one grouped control rather than three loose buttons —
 * they are one decision ("reconcile with the remote"), and grouping them is
 * what keeps the row from wrapping into a button soup at 280px.
 */

import { ArrowDown, ArrowUp, CloudOff, CloudUpload, Download, Loader2, RefreshCw, Upload } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import type { ReactNode } from "react";

import * as api from "../api";
import { BranchPopover } from "./BranchPopover";
import type { BranchPopoverHandle } from "./BranchPopover";
import { RepoSwitcher } from "./RepoSwitcher";
import type { RepoSwitcherHandle } from "./RepoSwitcher";
import { asAction } from "./gitFormat";
import { useGitPanelBridge } from "../state/GitPanelContext";
import type { GitViewHandlers } from "../state/GitPanelContext";
import type { GitActionRunner } from "../state/useGitAction";
import type { GitData } from "../state/useGitData";
import type { GitReposState } from "../state/useGitRepos";
import type { GitAction } from "../types";

export interface GitHeaderProps {
  repos: GitReposState;
  data: GitData;
  action: GitActionRunner;
}

export function GitHeader({ repos, data, action }: GitHeaderProps) {
  const scope = repos.scope;
  const sync = data.sync;
  const busy = action.pending !== null;

  const behind = sync?.behind ?? 0;
  const ahead = sync?.ahead ?? 0;
  const detached = data.branches?.detached ?? false;
  // Unborn HEAD has no upstream either, but there is nothing to publish yet,
  // so it must not be mistaken for "needs publishing".
  const noUpstream = Boolean(sync) && !sync?.upstream && !sync?.unborn && !detached;

  const pushLabel = noUpstream ? "Publish branch" : "Push";

  const run = (label: string, operation: () => Promise<GitAction>) => {
    void action.run(label, operation);
  };

  const branchPicker = useRef<BranchPopoverHandle | null>(null);
  const repoPicker = useRef<RepoSwitcherHandle | null>(null);
  const bridge = useGitPanelBridge();
  // One repository renders no switcher, so the command is left unregistered
  // rather than bound to a control that is not on screen.
  const multiRepo = repos.repos.length > 1;

  const handlers = useMemo<GitViewHandlers>(
    () => ({
      openBranchPicker: () => branchPicker.current?.open(),
      ...(multiRepo ? { openRepoPicker: () => repoPicker.current?.open() } : {}),
    }),
    [multiRepo],
  );

  useEffect(() => bridge?.register(handlers), [bridge, handlers]);

  return (
    <header className="gitp-header">
      <div className="gitp-header-identity">
        <RepoSwitcher ref={repoPicker} repos={repos} disabled={busy} />
        <BranchPopover ref={branchPicker} branches={data.branches} scope={scope} action={action} />

        <span className="gitp-sync-state">
          {behind > 0 ? (
            <span className="gitp-sync-count" title={`${behind} commits to pull`}>
              <ArrowDown className="gitp-icon" aria-hidden="true" />
              <span>{behind}</span>
              <span className="gitp-sr">commits behind</span>
            </span>
          ) : null}
          {ahead > 0 ? (
            <span className="gitp-sync-count" title={`${ahead} commits to push`}>
              <ArrowUp className="gitp-icon" aria-hidden="true" />
              <span>{ahead}</span>
              <span className="gitp-sr">commits ahead</span>
            </span>
          ) : null}
          {noUpstream ? (
            <span className="gitp-sync-noupstream" title="This branch is not tracking a remote branch">
              <CloudOff className="gitp-icon" aria-hidden="true" />
              <span>no upstream</span>
            </span>
          ) : null}
        </span>
      </div>

      <div className="gitp-header-actions">
        <div className="gitp-sync-group" role="group" aria-label="Sync with remote">
          <SyncButton
            label="Fetch"
            pendingLabel="fetch"
            action={action}
            icon={<Download className="gitp-icon" aria-hidden="true" />}
            title="Fetch from the remote without changing the working tree"
            onRun={() => run("fetch", () => api.fetchRemote(scope, { prune: true }))}
          />
          <SyncButton
            label="Pull"
            pendingLabel="pull"
            action={action}
            icon={<ArrowDown className="gitp-icon" aria-hidden="true" />}
            title={behind > 0 ? `Pull ${behind} commits` : "Pull from the remote"}
            onRun={() => run("pull", () => api.pull(scope))}
          />
          <SyncButton
            label={pushLabel}
            pendingLabel="push"
            action={action}
            data-publish={noUpstream ? "" : undefined}
            icon={
              noUpstream ? (
                <CloudUpload className="gitp-icon" aria-hidden="true" />
              ) : (
                <Upload className="gitp-icon" aria-hidden="true" />
              )
            }
            title={
              noUpstream
                ? "Push this branch and set it to track the remote"
                : ahead > 0
                  ? `Push ${ahead} commits`
                  : "Push to the remote"
            }
            onRun={() =>
              run("push", () =>
                api.push(scope, noUpstream ? { setUpstream: true, branch: sync?.branch } : {}),
              )
            }
          />
        </div>

        <button
          type="button"
          className="gitp-icon-btn"
          aria-label="Refresh git status"
          title="Refresh"
          disabled={busy || data.loading}
          onClick={() => run("refresh", asAction("Refreshed", () => data.refresh()))}
        >
          {action.pending === "refresh" || data.loading ? (
            <Loader2 className="gitp-icon gitp-spin" aria-hidden="true" />
          ) : (
            <RefreshCw className="gitp-icon" aria-hidden="true" />
          )}
        </button>
      </div>
    </header>
  );
}

interface SyncButtonProps {
  label: string;
  /** The `action.pending` value this button owns, for the spinner state. */
  pendingLabel: string;
  action: GitActionRunner;
  icon: ReactNode;
  title: string;
  onRun: () => void;
  "data-publish"?: string;
}

function SyncButton({ label, pendingLabel, action, icon, title, onRun, ...rest }: SyncButtonProps) {
  const inFlight = action.pending === pendingLabel;
  return (
    <button
      type="button"
      className="gitp-sync-btn"
      title={title}
      aria-label={label}
      disabled={action.pending !== null}
      onClick={onRun}
      {...rest}
    >
      {inFlight ? <Loader2 className="gitp-icon gitp-spin" aria-hidden="true" /> : icon}
      <span className="gitp-sync-btn-label">{label}</span>
    </button>
  );
}
