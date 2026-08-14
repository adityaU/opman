/**
 * The commit composer.
 *
 * It sits at the top of the changes view because writing the message is the
 * work; the file list below it is the evidence. Amending prefills the previous
 * subject so the common case — "I forgot a file" — needs no retyping, and the
 * two toggles are the only options git actually asks for at this level.
 */

import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { GitCommitVertical, Layers, PenLine } from "lucide-react";

/**
 * What the keybinding layer can ask of the composer. Both are things only this
 * component can do — the message lives in its own state, not the shell's.
 */
export interface CommitBoxHandle {
  /** Focus the message field and select what is there, ready to be replaced. */
  focus: () => void;
  /** Commit the current message, or do nothing when there is nothing to say. */
  submit: () => void;
}

export interface CommitBoxProps {
  /** Subject of HEAD, used to prefill an amend. Empty on an unborn branch. */
  previousSubject: string;
  stagedCount: number;
  trackedCount: number;
  pending: string | null;
  onCommit: (message: string, options: { amend: boolean; stageAll: boolean }) => void;
}

export const CommitBox = forwardRef<CommitBoxHandle, CommitBoxProps>(function CommitBox(
  { previousSubject, stagedCount, trackedCount, pending, onCommit }: CommitBoxProps,
  ref,
) {
  const field = useRef<HTMLTextAreaElement | null>(null);
  const [message, setMessage] = useState("");
  const [amend, setAmend] = useState(false);
  const [stageAll, setStageAll] = useState(false);
  const busy = pending !== null;

  // Turning amending on adopts the previous subject; turning it off gives the
  // draft back rather than leaving the old subject in a fresh commit.
  const [draft, setDraft] = useState("");
  useEffect(() => {
    if (!amend) return;
    setDraft(message);
    setMessage((current) => (current.trim() ? current : previousSubject));
    // Only when the toggle flips — re-running on every keystroke would fight the typist.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [amend]);

  const reset = () => {
    setMessage("");
    setAmend(false);
    setStageAll(false);
    setDraft("");
  };

  const submit = () => {
    if (!message.trim() || busy) return;
    onCommit(message.trim(), { amend, stageAll });
    reset();
  };

  // A ref rather than props: the shell dispatches these as one-off commands,
  // and modelling "focus now" as state would need a token to be cleared again.
  useImperativeHandle(
    ref,
    () => ({
      focus: () => {
        field.current?.focus();
        field.current?.select();
      },
      submit,
    }),
    // `submit` closes over the message, so the handle has to follow it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [message, amend, stageAll, busy],
  );

  const willCommit = stagedCount > 0 || stageAll || amend;

  return (
    <form
      className="gitp-commit-box"
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <textarea
        ref={field}
        className="gitp-commit-message"
        value={message}
        placeholder={amend ? "Amend the last commit message…" : "Describe this change…"}
        aria-label="Commit message"
        rows={3}
        disabled={busy}
        onChange={(event) => setMessage(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            submit();
          }
        }}
      />

      <div className="gitp-commit-controls">
        <label className="gitp-toggle">
          <input
            type="checkbox"
            checked={amend}
            disabled={busy || !previousSubject}
            onChange={(event) => {
              const next = event.target.checked;
              setAmend(next);
              if (!next) setMessage(draft);
            }}
          />
          <PenLine size={13} aria-hidden="true" />
          <span>Amend last commit</span>
        </label>

        <label className="gitp-toggle">
          <input
            type="checkbox"
            checked={stageAll}
            disabled={busy || trackedCount === 0}
            onChange={(event) => setStageAll(event.target.checked)}
          />
          <Layers size={13} aria-hidden="true" />
          <span>Stage all tracked</span>
        </label>

        <button
          type="submit"
          className="gitp-btn gitp-btn-primary gitp-commit-submit"
          disabled={busy || !message.trim()}
        >
          <GitCommitVertical size={14} aria-hidden="true" />
          {amend ? "Amend" : "Commit"}
        </button>
      </div>

      {!willCommit && message.trim() ? (
        <p className="gitp-commit-warning" aria-live="polite">
          Nothing is staged. Turn on “Stage all tracked”, or stage files below.
        </p>
      ) : null}
    </form>
  );
});
