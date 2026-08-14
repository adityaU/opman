/**
 * Worktrees: several branches checked out at once, side by side on disk.
 *
 * This is the section most people have never opened, so the empty state
 * explains the idea rather than just reporting that a list is empty. Prune
 * names the directories it will forget before it does it — "prune" is a word
 * that hides how much it can throw away.
 */

import { useMemo, useState } from "react";
import { FolderTree, Plus, Trash2 } from "lucide-react";

import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { GitData } from "../state/useGitData";
import type { GitActionRunner } from "../state/useGitAction";
import type { GitWorktree } from "../types";

export interface WorktreesViewProps {
  data: GitData;
  scope: string;
  action: GitActionRunner;
}

export function WorktreesView({ data, scope, action }: WorktreesViewProps) {
  const [path, setPath] = useState("");
  const [branch, setBranch] = useState("");
  const [create, setCreate] = useState(true);
  const [startPoint, setStartPoint] = useState("");
  const [target, setTarget] = useState<GitWorktree | null>(null);
  const [refusal, setRefusal] = useState<{ tree: GitWorktree; message: string } | null>(null);
  const [pruning, setPruning] = useState(false);

  const busy = action.pending !== null;
  const prunable = useMemo(() => data.worktrees.filter((tree) => tree.prunable), [data.worktrees]);

  const add = async () => {
    const trimmedPath = path.trim();
    const trimmedBranch = branch.trim();
    if (!trimmedPath || !trimmedBranch) return;
    const result = await action.run("add worktree", () =>
      api.addWorktree(scope, trimmedPath, trimmedBranch, {
        create,
        startPoint: create ? startPoint.trim() || undefined : undefined,
      }),
    );
    if (result?.ok) {
      setPath("");
      setBranch("");
      setStartPoint("");
    }
  };

  const remove = async (tree: GitWorktree, force: boolean) => {
    setTarget(null);
    setRefusal(null);
    const result = await action.run(force ? "force remove worktree" : "remove worktree", () =>
      api.removeWorktree(scope, tree.path, force),
    );
    if (result && !result.ok && !force) {
      setRefusal({ tree, message: result.hint ? `${result.message} — ${result.hint}` : result.message });
    }
  };

  return (
    <div className="gitp-worktrees">
      <form
        className="gitp-create"
        onSubmit={(event) => {
          event.preventDefault();
          void add();
        }}
      >
        <div className="gitp-create-fields">
          <input
            className="gitp-input"
            value={path}
            placeholder="Path for the new worktree"
            aria-label="Path for the new worktree"
            autoComplete="off"
            spellCheck={false}
            disabled={busy}
            onChange={(event) => setPath(event.target.value)}
          />
          <input
            className="gitp-input gitp-input-narrow"
            value={branch}
            placeholder="Branch"
            aria-label="Branch to check out in the worktree"
            autoComplete="off"
            spellCheck={false}
            disabled={busy}
            onChange={(event) => setBranch(event.target.value)}
          />
        </div>
        <div className="gitp-create-controls">
          <label className="gitp-toggle">
            <input
              type="checkbox"
              checked={create}
              disabled={busy}
              onChange={(event) => setCreate(event.target.checked)}
            />
            <span>Create this branch</span>
          </label>
          {create ? (
            <input
              className="gitp-input gitp-input-narrow"
              value={startPoint}
              placeholder="from HEAD"
              aria-label="Start point for the new branch"
              autoComplete="off"
              spellCheck={false}
              disabled={busy}
              onChange={(event) => setStartPoint(event.target.value)}
            />
          ) : null}
          <button
            type="submit"
            className="gitp-btn gitp-btn-primary"
            disabled={busy || !path.trim() || !branch.trim()}
          >
            <Plus size={14} aria-hidden="true" />
            Add worktree
          </button>
        </div>
      </form>

      <section className="gitp-section" aria-label="Worktrees">
        <h3 className="gitp-section-title">
          Worktrees <span className="gitp-count">{data.worktrees.length}</span>
          <button
            type="button"
            className="gitp-btn gitp-btn-quiet gitp-section-action"
            disabled={busy || prunable.length === 0}
            title={
              prunable.length
                ? `Forget ${prunable.length} worktree record${prunable.length === 1 ? "" : "s"} whose directory is gone`
                : "Nothing to prune: every worktree still exists on disk"
            }
            onClick={() => setPruning(true)}
          >
            Prune
          </button>
        </h3>

        {data.worktrees.length ? (
          <ul className="gitp-list">
            {data.worktrees.map((tree) => (
              <li className="gitp-worktree-row" key={tree.path} data-current={tree.current ? "" : undefined}>
                <div className="gitp-worktree-main">
                  <div className="gitp-worktree-line">
                    <span className="gitp-worktree-path gitp-mono">{tree.relative ?? tree.path}</span>
                    {tree.main ? <span className="gitp-badge">main</span> : null}
                    {tree.current ? <span className="gitp-badge gitp-badge-current">current</span> : null}
                    {tree.locked ? <span className="gitp-badge gitp-badge-locked">locked</span> : null}
                    {tree.prunable ? (
                      <span className="gitp-badge gitp-badge-prunable" title={tree.prunable}>
                        prunable
                      </span>
                    ) : null}
                  </div>
                  <div className="gitp-worktree-sub">
                    <span className="gitp-worktree-branch">{tree.branch ?? "detached"}</span>
                    <span className="gitp-worktree-head gitp-mono">{tree.head.slice(0, 7)}</span>
                  </div>
                </div>
                <div className="gitp-row-actions">
                  <button
                    type="button"
                    className="gitp-icon-btn gitp-icon-btn-danger"
                    disabled={busy || tree.main || tree.current}
                    aria-label={`Remove worktree ${tree.relative ?? tree.path}`}
                    title={
                      tree.main
                        ? "The main worktree cannot be removed"
                        : tree.current
                          ? "This is the worktree you are in"
                          : `Remove worktree ${tree.relative ?? tree.path}`
                    }
                    onClick={() => setTarget(tree)}
                  >
                    <Trash2 size={14} aria-hidden="true" />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <div className="gitp-empty gitp-empty-rich">
            <FolderTree size={20} aria-hidden="true" />
            <p className="gitp-empty-title">No linked worktrees</p>
            <p className="gitp-empty-body">
              A worktree is a second directory backed by this same repository, with a different branch
              checked out. Add one to review a pull request, run a long build, or keep an experiment going
              without stashing what you are working on right now.
            </p>
          </div>
        )}
      </section>

      <ConfirmDialog
        open={target !== null}
        title="Remove worktree"
        danger
        confirmLabel="Remove worktree"
        body={
          <p className="gitp-confirm-text">
            The directory <code className="gitp-mono">{target?.path}</code> will be deleted and git will stop
            tracking it. The branch <code className="gitp-mono">{target?.branch ?? "(detached)"}</code> stays.
          </p>
        }
        onCancel={() => setTarget(null)}
        onConfirm={() => {
          if (target) void remove(target, false);
        }}
      />

      <ConfirmDialog
        open={refusal !== null}
        title="Git refused to remove this worktree"
        danger
        confirmLabel="Remove anyway"
        requireTyped={refusal?.tree.relative ?? refusal?.tree.path}
        body={
          <>
            <p className="gitp-confirm-text">{refusal?.message}</p>
            <p className="gitp-confirm-text gitp-confirm-note">
              Removing anyway destroys the uncommitted work in that directory. It cannot be recovered.
            </p>
          </>
        }
        onCancel={() => setRefusal(null)}
        onConfirm={() => {
          if (refusal) void remove(refusal.tree, true);
        }}
      />

      <ConfirmDialog
        open={pruning}
        title="Prune worktree records"
        danger
        confirmLabel="Prune"
        body={
          <>
            <p className="gitp-confirm-text">
              Git will forget {prunable.length} worktree record{prunable.length === 1 ? "" : "s"} whose
              directory no longer exists:
            </p>
            <ul className="gitp-confirm-list">
              {prunable.map((tree) => (
                <li key={tree.path} className="gitp-mono">
                  {tree.relative ?? tree.path}
                </li>
              ))}
            </ul>
          </>
        }
        onCancel={() => setPruning(false)}
        onConfirm={() => {
          setPruning(false);
          void action.run("prune worktrees", () => api.pruneWorktrees(scope));
        }}
      />
    </div>
  );
}
