/**
 * The working tree: compose a commit, then see what would go into it.
 *
 * Order is intentional. The composer is first because writing the message is
 * the task; the three groups below answer "and what exactly am I committing?"
 * in the order git itself cares about — staged, changed, untracked.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Sparkles } from "lucide-react";

import * as api from "../api";
import { CommitBox } from "../components/CommitBox";
import type { CommitBoxHandle } from "../components/CommitBox";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { FileGroup } from "../components/FileGroup";
import { asAction, subjectOf } from "../components/gitFormat";
import { useGitPanelBridge } from "../state/GitPanelContext";
import type { GitViewHandlers } from "../state/GitPanelContext";
import type { GitActionRunner } from "../state/useGitAction";
import type { GitData } from "../state/useGitData";

export interface ChangesViewProps {
  data: GitData;
  scope: string;
  action: GitActionRunner;
  onSendToAI?: (prompt: string) => void;
}

interface OpenDiff {
  path: string;
  staged: boolean;
  text: string | null;
}

export function ChangesView({ data, scope, action, onSendToAI }: ChangesViewProps) {
  const status = data.status;
  const [open, setOpen] = useState<OpenDiff | null>(null);
  const [loadingDiff, setLoadingDiff] = useState(false);
  const [discarding, setDiscarding] = useState<string[] | null>(null);
  const busy = action.pending !== null;

  const openDiff = useCallback(
    (path: string, staged: boolean) => {
      setOpen((current) => (current?.path === path ? null : { path, staged, text: null }));
      if (open?.path === path) return;
      setLoadingDiff(true);
      void api
        .fetchDiff(scope, path, staged)
        .then((response) => setOpen((current) => (current?.path === path ? { ...current, text: response.diff } : current)))
        .catch((cause: unknown) => {
          console.error("git diff failed", cause);
          setOpen((current) => (current?.path === path ? { ...current, text: "" } : current));
        })
        .finally(() => setLoadingDiff(false));
    },
    [open?.path, scope],
  );

  const stage = (files: string[]) =>
    void action.run("stage", asAction(`Staged ${files.length} file(s)`, () => api.stage(scope, files)));
  const unstage = (files: string[]) =>
    void action.run("unstage", asAction(`Unstaged ${files.length} file(s)`, () => api.unstage(scope, files)));

  const commit = (message: string, options: { amend: boolean; stageAll: boolean }) => {
    void action.run("commit", () => api.commit(scope, message, options));
  };

  const sendToAI = useCallback(() => {
    if (!onSendToAI) return;
    void api
      .fetchDiff(scope)
      .then((response) => {
        const diff = response.diff.trim();
        onSendToAI(
          diff
            ? `Review these uncommitted changes and tell me what they do and anything that looks wrong:\n\n\`\`\`diff\n${diff}\n\`\`\``
            : "There are no uncommitted changes in this repository.",
        );
      })
      .catch((cause: unknown) => console.error("git diff failed", cause));
  }, [onSendToAI, scope]);

  const generateCommitMessage = useCallback(() => {
    if (!onSendToAI) return;
    void api
      .fetchDiff(scope, undefined, true)
      .then((response) => {
        const diff = response.diff.trim();
        onSendToAI(
          diff
            ? `Write a commit message for these staged changes. Reply with the message only:\n\n\`\`\`diff\n${diff}\n\`\`\``
            : "Nothing is staged, so there is no commit message to write yet.",
        );
      })
      .catch((cause: unknown) => console.error("git diff failed", cause));
  }, [onSendToAI, scope]);

  const composer = useRef<CommitBoxHandle | null>(null);
  const bridge = useGitPanelBridge();
  const selection = bridge?.selection;

  const handlers = useMemo<GitViewHandlers>(
    () => ({
      openDiff,
      focusCommitMessage: () => composer.current?.focus(),
      commit: () => composer.current?.submit(),
      sendToReview: sendToAI,
      // Left unregistered without a chat to send to, so the key stays a no-op
      // rather than quietly doing nothing at a layer the user cannot see.
      ...(onSendToAI ? { generateCommitMessage } : {}),
    }),
    [openDiff, sendToAI, generateCommitMessage, onSendToAI],
  );

  useEffect(() => bridge?.register(handlers), [bridge, handlers]);

  if (!status) return null;

  const tracked = [...status.staged, ...status.unstaged];
  const total = tracked.length + status.untracked.length;

  const groupProps = {
    openPath: open?.path ?? null,
    openDiff: open?.text ?? null,
    diffLoading: loadingDiff,
    onOpenDiff: openDiff,
    onStage: (path: string) => stage([path]),
    onUnstage: (path: string) => unstage([path]),
    disabled: busy,
    selection,
  };

  return (
    <div className="gitp-changes">
      <CommitBox
        ref={composer}
        previousSubject={data.log.length > 0 ? subjectOf(data.log[0].message) : ""}
        stagedCount={status.staged.length}
        trackedCount={tracked.length}
        pending={action.pending}
        onCommit={commit}
      />

      {total === 0 ? (
        <EmptyChanges branch={status.branch} />
      ) : (
        <>
          <FileGroup
            {...groupProps}
            title="Staged"
            files={status.staged}
            staged
            variant="staged"
            bulkLabel="Unstage all"
            onBulk={() => unstage(status.staged.map((file) => file.path))}
            extra={
              onSendToAI ? (
                <button type="button" className="gitp-btn gitp-btn-quiet" disabled={busy} onClick={sendToAI}>
                  <Sparkles size={13} aria-hidden="true" />
                  Ask AI about this diff
                </button>
              ) : undefined
            }
          />

          <FileGroup
            {...groupProps}
            title="Changed"
            files={status.unstaged}
            staged={false}
            variant="unstaged"
            bulkLabel="Stage all"
            onBulk={() => stage(status.unstaged.map((file) => file.path))}
            onDiscard={(path) => setDiscarding([path])}
            extra={
              onSendToAI && status.staged.length === 0 ? (
                <button type="button" className="gitp-btn gitp-btn-quiet" disabled={busy} onClick={sendToAI}>
                  <Sparkles size={13} aria-hidden="true" />
                  Ask AI about this diff
                </button>
              ) : undefined
            }
          />

          <FileGroup
            {...groupProps}
            title="Untracked"
            files={status.untracked}
            staged={false}
            variant="untracked"
            bulkLabel="Stage all"
            onBulk={() => stage(status.untracked.map((file) => file.path))}
            onDiscard={(path) => setDiscarding([path])}
          />
        </>
      )}

      <ConfirmDialog
        open={discarding !== null}
        title="Discard changes?"
        body={
          <>
            <p>
              {discarding?.length === 1
                ? `All changes to ${discarding[0]} will be thrown away.`
                : `Changes to ${discarding?.length ?? 0} files will be thrown away.`}
            </p>
            <p>Git keeps no copy of uncommitted work. This cannot be undone.</p>
          </>
        }
        confirmLabel="Discard"
        danger
        requireTyped={undefined}
        onConfirm={() => {
          const files = discarding ?? [];
          setDiscarding(null);
          setOpen(null);
          void action.run("discard", asAction(`Discarded ${files.length} file(s)`, () => api.discard(scope, files)));
        }}
        onCancel={() => setDiscarding(null)}
      />
    </div>
  );
}

/** The clean-tree state, used to explain the panel rather than confirm nothing. */
function EmptyChanges({ branch }: { branch: string }) {
  return (
    <div className="gitp-empty">
      <p className="gitp-empty-title">Your working tree is clean.</p>
      <p className="gitp-empty-body">
        Every file on <span className="gitp-mono">{branch}</span> matches the last commit. As you edit, files show up
        here in three groups: <strong>Untracked</strong> for files git has never seen, <strong>Changed</strong> for
        edits to files it tracks, and <strong>Staged</strong> for what the next commit will contain. Stage what belongs
        together, write a message above, and commit.
      </p>
    </div>
  );
}
