import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { FolderGit2, LayoutGrid, MessagesSquare, Search } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { advance, currentStep, EMPTY_DRAFT, isComplete, retreat, type OpenerChoice, type OpenerDraft, type StepId } from "./steps";
import { Crumbs, RowBody, rowClass } from "./rows";

/**
 * The staged widget picker: kind → project → session.
 *
 * One list that changes what it is listing, rather than three dialogs: the
 * head names the question, Enter goes forward, Backspace on an empty filter
 * goes back. Everything is reachable by typing — the pointer is optional, and
 * the list is filtered rather than paged so the answer is always one substring
 * away.
 *
 * The three stages deliberately do not look alike. rows.tsx gives each its own
 * row anatomy, and the head carries the stage's glyph, so "which question is
 * this" is answered by the shape of the surface before any label is read.
 */

export type { OpenerChoice };

export interface WidgetOpenerProps {
  /** Choices for the step the draft is on. */
  readonly choicesFor: (step: StepId, draft: OpenerDraft) => readonly OpenerChoice[];
  /**
   * Where to start. An empty pane's four buttons have already answered step
   * one, so re-asking it would throw away the click the user just made.
   */
  readonly initialDraft?: OpenerDraft;
  readonly onDone: (draft: OpenerDraft) => void;
  readonly onCancel: () => void;
}

const STEP_TITLE: Readonly<Record<StepId, string>> = {
  kind: "Open what?",
  project: "In which project?",
  session: "Which session?",
};

/** Says the stage in the first glyph the eye reaches, ahead of the title. */
const STEP_ICON: Readonly<Record<StepId, LucideIcon>> = {
  kind: LayoutGrid,
  project: FolderGit2,
  session: MessagesSquare,
};

const STEP_FILTER: Readonly<Record<StepId, string>> = {
  kind: "Filter widgets",
  project: "Filter projects",
  session: "Filter sessions",
};

/** What the empty state is empty *of* — "nothing matches" named nothing. */
const STEP_NOUN: Readonly<Record<StepId, string>> = {
  kind: "widgets",
  project: "projects",
  session: "sessions",
};

export const WidgetOpener: React.FC<WidgetOpenerProps> = function WidgetOpener({
  choicesFor,
  initialDraft,
  onDone,
  onCancel,
}) {
  const [draft, setDraft] = useState<OpenerDraft>(initialDraft ?? EMPTY_DRAFT);
  const [filter, setFilter] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  const step = currentStep(draft);

  const choices = useMemo(() => {
    if (step === null) return [];
    const all = choicesFor(step, draft);
    const needle = filter.trim().toLowerCase();
    if (!needle) return all;
    return all.filter((choice) => choice.label.toLowerCase().includes(needle));
  }, [choicesFor, draft, filter, step]);

  // A finished draft resolves on the next tick rather than mid-render.
  useEffect(() => {
    if (!isComplete(draft)) return;
    onDone(draft);
  }, [draft, onDone]);

  useEffect(() => setCursor(0), [step, filter]);
  useEffect(() => inputRef.current?.focus(), [step]);

  // A long session list scrolls, and a cursor the arrows have driven past the
  // fold is a cursor the user has lost.
  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(".wsp-opener-row.is-cursor")
      ?.scrollIntoView({ block: "nearest" });
  }, [cursor, choices]);

  const choose = useCallback((value: string | null) => {
    setDraft((current) => advance(current, value));
    setFilter("");
  }, []);

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      const move = (delta: number) =>
        setCursor((c) => (choices.length === 0 ? 0 : (c + delta + choices.length) % choices.length));

      // Ctrl+n/p alongside the arrows: this is a picker vim users will live in.
      if (event.key === "ArrowDown" || (event.ctrlKey && event.key === "n")) move(1);
      else if (event.key === "ArrowUp" || (event.ctrlKey && event.key === "p")) move(-1);
      else if (event.key === "Enter") {
        const picked = choices[cursor];
        if (picked) choose(picked.value);
      } else if (event.key === "Escape") onCancel();
      else if (event.key === "Backspace" && filter === "") {
        // Only when the filter is empty, so backspace still edits text first.
        if (draft.kind === null) onCancel();
        else setDraft(retreat);
      } else if (event.key === "Tab") {
        if (event.shiftKey) setDraft(retreat);
        else {
          const picked = choices[cursor];
          if (picked) choose(picked.value);
        }
      } else return;

      event.preventDefault();
      event.stopPropagation();
    },
    [choices, choose, cursor, draft.kind, filter, onCancel],
  );

  if (step === null) return null;

  const StepIcon = STEP_ICON[step];
  // Only the last step of a draft actually opens anything; the rest go on.
  const commits = step === "session" || (step === "project" && draft.kind !== "chat");

  return createPortal(
    <div className="modal-backdrop wsp-opener-backdrop" onClick={onCancel}>
      <div
        className="modal-dialog-surface wsp-opener"
        role="dialog"
        aria-modal="true"
        aria-label="Open widget in pane"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="wsp-opener-head">
          <span className="wsp-opener-stage" aria-hidden="true">
            <StepIcon size={14} />
          </span>
          <span className="wsp-opener-title">{STEP_TITLE[step]}</span>
          {/* Only once the list has outgrown the surface: below that the count
              is visibly true already, and a "4" next to four rows is noise. */}
          {choices.length > 8 && <span className="wsp-opener-count">{choices.length}</span>}
        </div>

        <Crumbs draft={draft} />

        <div className="wsp-opener-search">
          <Search size={14} aria-hidden="true" />
          <input
            ref={inputRef}
            className="wsp-opener-input"
            value={filter}
            placeholder={STEP_FILTER[step]}
            aria-label={`${STEP_TITLE[step]} ${STEP_FILTER[step]}`}
            onChange={(event) => setFilter(event.target.value)}
            onKeyDown={onKeyDown}
          />
        </div>

        <ul className="wsp-opener-list" ref={listRef} role="listbox" aria-label={STEP_TITLE[step]}>
          {choices.length === 0 && (
            <li className="wsp-opener-empty">
              {filter.trim()
                ? `No ${STEP_NOUN[step]} match “${filter.trim()}” — backspace to clear`
                : `No ${STEP_NOUN[step]} yet`}
            </li>
          )}
          {choices.map((choice, index) => (
            <React.Fragment key={choice.value ?? "__new__"}>
              {step === "session" && index === 1 && choices[0].value === null && (
                <li className="wsp-opener-group" role="presentation">
                  <span>Recent</span>
                  <span className="wsp-opener-group-note">newest first</span>
                </li>
              )}
              <li>
                <button
                  type="button"
                  role="option"
                  aria-selected={index === cursor}
                  className={`${rowClass(step, choice)}${index === cursor ? " is-cursor" : ""}`}
                  onMouseEnter={() => setCursor(index)}
                  onClick={() => choose(choice.value)}
                >
                  <RowBody step={step} choice={choice} />
                </button>
              </li>
            </React.Fragment>
          ))}
        </ul>

        <div className="wsp-opener-keys">
          <kbd>↑</kbd> <kbd>↓</kbd> move · <kbd>↵</kbd> {commits ? "open" : "next"} ·{" "}
          {draft.kind !== null && (
            <>
              <kbd>⌫</kbd> back ·{" "}
            </>
          )}
          <kbd>esc</kbd> close
        </div>
      </div>
    </div>,
    document.body,
  );
};
