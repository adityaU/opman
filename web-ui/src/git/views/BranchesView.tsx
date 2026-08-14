/**
 * Branches: create, switch, integrate, retire.
 *
 * Deletion is the only operation here a person cannot undo from the panel, so
 * it is the only one that stops for a dialog — and when git refuses because
 * the branch is unmerged, the refusal itself is shown and the forced retry is
 * offered as a second, separate decision rather than a checkbox nobody reads.
 */

import { useMemo, useState } from "react";
import { GitBranch as GitBranchIcon, Plus, Search } from "lucide-react";

import * as api from "../api";
import { BranchRow } from "../components/BranchRow";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { GitData } from "../state/useGitData";
import type { GitActionRunner } from "../state/useGitAction";
import type { GitBranch } from "../types";

export interface BranchesViewProps {
  data: GitData;
  scope: string;
  action: GitActionRunner;
}

/** A remote branch is addressed as `<remote>/<name>` on the wire. */
function splitRemote(branch: GitBranch): { name: string; remote?: string } {
  if (!branch.remote) return { name: branch.name };
  const cut = branch.name.indexOf("/");
  if (cut < 0) return { name: branch.name };
  return { remote: branch.name.slice(0, cut), name: branch.name.slice(cut + 1) };
}

export function BranchesView({ data, scope, action }: BranchesViewProps) {
  const [name, setName] = useState("");
  const [startPoint, setStartPoint] = useState("");
  const [switchTo, setSwitchTo] = useState(true);
  const [filter, setFilter] = useState("");
  const [target, setTarget] = useState<GitBranch | null>(null);
  const [refusal, setRefusal] = useState<{ branch: GitBranch; message: string } | null>(null);

  const busy = action.pending !== null;
  const local = data.branches?.local ?? [];
  const remote = data.branches?.remote ?? [];

  const needle = filter.trim().toLowerCase();
  const match = (branch: GitBranch) => !needle || branch.name.toLowerCase().includes(needle);
  const shownLocal = useMemo(() => local.filter(match), [local, needle]);
  const shownRemote = useMemo(() => remote.filter(match), [remote, needle]);
  const hidden = local.length + remote.length - shownLocal.length - shownRemote.length;

  const create = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    const result = await action.run("create branch", () =>
      api.createBranch(scope, trimmed, {
        startPoint: startPoint.trim() || undefined,
        checkout: switchTo,
      }),
    );
    if (result?.ok) {
      setName("");
      setStartPoint("");
    }
  };

  const remove = async (branch: GitBranch, force: boolean) => {
    const { name: bare, remote: origin } = splitRemote(branch);
    setTarget(null);
    setRefusal(null);
    const result = await action.run(force ? "force delete branch" : "delete branch", () =>
      api.deleteBranch(scope, bare, { force, remote: origin }),
    );
    if (result && !result.ok && !force) {
      setRefusal({ branch, message: result.hint ? `${result.message} — ${result.hint}` : result.message });
    }
  };

  const rows = (list: GitBranch[]) =>
    list.map((branch) => (
      <BranchRow
        key={`${branch.remote ? "r" : "l"}:${branch.name}`}
        branch={branch}
        busy={busy}
        onCheckout={() => void action.run("checkout", () => api.checkout(scope, branch.name))}
        onMerge={() => void action.run("merge", () => api.merge(scope, branch.name))}
        onRename={(to) => void action.run("rename branch", () => api.renameBranch(scope, branch.name, to))}
        onDelete={() => setTarget(branch)}
      />
    ));

  return (
    <div className="gitp-branches">
      <form
        className="gitp-create"
        onSubmit={(event) => {
          event.preventDefault();
          void create();
        }}
      >
        <div className="gitp-create-fields">
          <input
            className="gitp-input"
            value={name}
            placeholder="New branch name"
            aria-label="New branch name"
            autoComplete="off"
            spellCheck={false}
            disabled={busy}
            onChange={(event) => setName(event.target.value)}
          />
          <input
            className="gitp-input gitp-input-narrow"
            value={startPoint}
            placeholder={data.branches?.current ? `from ${data.branches.current}` : "from HEAD"}
            aria-label="Start point (defaults to the current HEAD)"
            autoComplete="off"
            spellCheck={false}
            disabled={busy}
            onChange={(event) => setStartPoint(event.target.value)}
          />
        </div>
        <div className="gitp-create-controls">
          <label className="gitp-toggle">
            <input
              type="checkbox"
              checked={switchTo}
              disabled={busy}
              onChange={(event) => setSwitchTo(event.target.checked)}
            />
            <span>Check out after creating</span>
          </label>
          <button type="submit" className="gitp-btn gitp-btn-primary" disabled={busy || !name.trim()}>
            <Plus size={14} aria-hidden="true" />
            New branch
          </button>
        </div>
      </form>

      <div className="gitp-filter">
        <Search size={14} aria-hidden="true" className="gitp-filter-icon" />
        <input
          className="gitp-input gitp-filter-input"
          value={filter}
          placeholder="Filter branches"
          aria-label="Filter branches"
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => setFilter(event.target.value)}
        />
        {needle && hidden > 0 ? (
          <span className="gitp-filter-hidden" aria-live="polite">
            {hidden} hidden
          </span>
        ) : null}
      </div>

      <section className="gitp-section" aria-label="Local branches">
        <h3 className="gitp-section-title">
          Local <span className="gitp-count">{shownLocal.length}</span>
        </h3>
        {shownLocal.length ? (
          <ul className="gitp-list">{rows(shownLocal)}</ul>
        ) : (
          <p className="gitp-empty">
            <GitBranchIcon size={16} aria-hidden="true" />
            {needle ? "No local branch matches this filter." : "No local branches yet."}
          </p>
        )}
      </section>

      <section className="gitp-section" aria-label="Remote branches">
        <h3 className="gitp-section-title">
          Remote <span className="gitp-count">{shownRemote.length}</span>
        </h3>
        {shownRemote.length ? (
          <ul className="gitp-list">{rows(shownRemote)}</ul>
        ) : (
          <p className="gitp-empty">
            {needle ? "No remote branch matches this filter." : "No remote branches. Fetch to see them."}
          </p>
        )}
      </section>

      <ConfirmDialog
        open={target !== null}
        title="Delete branch"
        danger
        confirmLabel="Delete branch"
        requireTyped={target?.remote ? target.name : undefined}
        body={
          <p className="gitp-confirm-text">
            <code className="gitp-mono">{target?.name}</code> will be deleted
            {target?.remote ? " on the remote, for everyone" : " from this repository"}. Commits it alone
            points at become unreachable.
          </p>
        }
        onCancel={() => setTarget(null)}
        onConfirm={() => {
          if (target) void remove(target, false);
        }}
      />

      <ConfirmDialog
        open={refusal !== null}
        title="Git refused to delete this branch"
        danger
        confirmLabel="Delete anyway"
        body={
          <>
            <p className="gitp-confirm-text">{refusal?.message}</p>
            <p className="gitp-confirm-text gitp-confirm-note">
              Deleting anyway discards any commit on{" "}
              <code className="gitp-mono">{refusal?.branch.name}</code> that is not merged elsewhere.
            </p>
          </>
        }
        onCancel={() => setRefusal(null)}
        onConfirm={() => {
          if (refusal) void remove(refusal.branch, true);
        }}
      />
    </div>
  );
}
