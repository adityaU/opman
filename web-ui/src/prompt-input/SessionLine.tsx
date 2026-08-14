/**
 * The composer's dateline: who you are writing to, and what they are doing.
 *
 * A pane's header is only there when you peek at it, so the session's name had
 * nowhere permanent to live — with two chat panes open, the composer you are
 * typing into did not say which conversation it belonged to. The name now sits
 * on the composer's own top line, where the caret already is.
 *
 * That line is also the session's status line. The live progress text used to
 * own a row of its own, so a turn starting made the card taller; here it shares
 * the dateline — identity on the left, what the runner is doing on the right.
 * Nothing moves when a turn begins.
 *
 * The name is editable in place. Renaming a conversation belongs where you are
 * having it, not only in the sidebar's context menu.
 */
import React, { useCallback, useEffect, useRef, useState } from "react";
import { Pencil } from "lucide-react";
import { renameSession } from "../api";

interface Props {
  /** Server-side title. Null when the pane has no session yet. */
  readonly title: string | null;
  readonly sessionId: string | null;
  readonly busy: boolean;
  /** Latest runner progress line, shown on the right of the same row. */
  readonly progressText?: string | null;
}

export function SessionLine({ title, sessionId, busy, progressText }: Props) {
  const [draft, setDraft] = useState<string | null>(null);
  /** Set the moment a rename is sent, so the new name shows before the server echoes it. */
  const [renamed, setRenamed] = useState<{ id: string; title: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const rowRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // A pane rebound to another session must not keep the previous one's edits.
  useEffect(() => {
    setDraft(null);
    setSaving(false);
  }, [sessionId]);

  const shown = (renamed?.id === sessionId ? renamed.title : null) || title;
  const editable = !!sessionId;

  const commit = useCallback(async () => {
    const next = draft?.trim();
    setDraft(null);
    if (!sessionId || !next || next === shown) return;
    setSaving(true);
    setRenamed({ id: sessionId, title: next });
    try {
      await renameSession(sessionId, next);
    } catch {
      setRenamed(null);
    } finally {
      setSaving(false);
    }
  }, [draft, sessionId, shown]);

  /*
   * A pane claims DOM focus for itself the first time you press inside it, which
   * lands after this input mounts and would take the caret straight back out.
   * Focusing on the next frame puts it where the click asked for it, and editing
   * ends on an outside press rather than on blur so a stolen focus cannot close
   * the field mid-rename.
   */
  useEffect(() => {
    if (draft === null) return;
    const frame = requestAnimationFrame(() => inputRef.current?.focus());
    const onOutside = (event: PointerEvent) => {
      if (rowRef.current?.contains(event.target as Node)) return;
      void commit();
    };
    // Escape is claimed at capture phase by the shell's keymap, so the input's
    // own handler never sees it. Cancelling has to be caught at the same phase.
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      setDraft(null);
    };
    window.addEventListener("pointerdown", onOutside, true);
    window.addEventListener("keydown", onEscape, true);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("pointerdown", onOutside, true);
      window.removeEventListener("keydown", onEscape, true);
    };
  }, [draft === null, commit]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key !== "Enter") return;
      event.preventDefault();
      void commit();
    },
    [commit],
  );

  return (
    <div className="composer-session" ref={rowRef}>
      <span
        className="composer-session-mark"
        data-state={busy ? "busy" : "idle"}
        aria-hidden="true"
      />
      {draft === null ? (
        <button
          type="button"
          className="composer-session-name"
          onClick={() => editable && setDraft(shown || "")}
          disabled={!editable || saving}
          title={editable ? "Rename this session" : undefined}
          aria-label={shown ? `Session: ${shown}. Rename` : "Name this session"}
        >
          <span className={`composer-session-title${shown ? "" : " is-unnamed"}`}>
            {shown || "New session"}
          </span>
          {editable && <Pencil className="composer-session-pencil" size={10} aria-hidden="true" />}
        </button>
      ) : (
        <input
          ref={inputRef}
          className="composer-session-input"
          value={draft}
          autoFocus
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={handleKeyDown}
          // Select all, but keep the first character in view: select() alone
          // scrolls a long name to its end, which reads as truncation.
          onFocus={(event) => {
            const field = event.currentTarget;
            field.setSelectionRange(0, field.value.length);
            field.scrollLeft = 0;
          }}
          placeholder="Name this session"
          aria-label="Session name"
          maxLength={120}
        />
      )}
      {progressText && (
        <span className="composer-session-progress" role="status" aria-live="polite">
          <span className="composer-session-working">Working</span>
          <span className="composer-session-progress-text" title={progressText}>
            {progressText}
          </span>
          <span className="composer-session-dots" aria-hidden="true"><i /><i /><i /></span>
        </span>
      )}
    </div>
  );
}
