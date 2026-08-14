/**
 * One changed file.
 *
 * The row itself opens the diff, because reading the change is the thing a
 * person wants most often; staging and discarding are hover-revealed so the
 * list stays a list of files rather than a grid of buttons.
 */

import { useEffect, useRef } from "react";
import { FileDiff, Minus, Plus, Undo2 } from "lucide-react";

import { splitPath, statusLabel } from "./gitFormat";
import type { GitSectionVariant, GitSelection } from "../state/useGitSelection";

export interface FileRowProps {
  path: string;
  status: string;
  /** True when the file is already staged, which flips the stage control. */
  staged: boolean;
  /** True when this row's diff is open below it. */
  selected?: boolean;
  /** Keyboard cursor, absent when the row is rendered outside the panel. */
  selection?: GitSelection;
  /** Which section this row belongs to, for the cursor's identity. */
  variant?: GitSectionVariant;
  disabled?: boolean;
  onOpenDiff: (path: string, staged: boolean) => void;
  onStage: (path: string) => void;
  onUnstage: (path: string) => void;
  /** Omitted for files git can restore no other way (nothing to discard to). */
  onDiscard?: (path: string) => void;
}

/**
 * True when the element is already wholly on screen inside its scroller.
 *
 * Clicking a row selects it too, and scrolling on every click would yank the
 * list under the pointer; only a cursor that has moved somewhere unseen — which
 * is what the keyboard does — is worth a scroll.
 */
function fullyVisible(element: HTMLElement): boolean {
  const rect = element.getBoundingClientRect();
  let top = 0;
  let bottom = window.innerHeight;
  for (let node = element.parentElement; node; node = node.parentElement) {
    if (node.scrollHeight > node.clientHeight) {
      const box = node.getBoundingClientRect();
      top = box.top;
      bottom = box.bottom;
      break;
    }
  }
  return rect.top >= top && rect.bottom <= bottom;
}

export function FileRow({
  path,
  status,
  staged,
  selected,
  selection,
  variant,
  disabled,
  onOpenDiff,
  onStage,
  onUnstage,
  onDiscard,
}: FileRowProps) {
  const { dir, name } = splitPath(path);
  const letter = status.trim().charAt(0) || "?";
  const row = useRef<HTMLDivElement | null>(null);

  const cursored = variant ? (selection?.isSelected(path, variant) ?? false) : false;
  // Without a selection to consult, an open diff is the only thing "selected"
  // can mean, which keeps the row usable standalone.
  const marked = selection && variant ? cursored : Boolean(selected);

  useEffect(() => {
    const element = row.current;
    if (!cursored || !element) return;
    if (typeof element.scrollIntoView !== "function") return;
    if (fullyVisible(element)) return;
    element.scrollIntoView({ block: "nearest" });
  }, [cursored]);

  const openDiff = () => {
    if (selection && variant) selection.select(path, variant);
    onOpenDiff(path, staged);
  };

  return (
    <div ref={row} className="gitp-file-row" data-selected={marked ? "" : undefined}>
      <button
        type="button"
        className="gitp-file-main"
        onClick={openDiff}
        aria-expanded={selected ?? false}
      >
        <span className="gitp-file-status" title={statusLabel(status)} data-status={letter}>
          {letter}
        </span>
        <span className="gitp-file-path">
          {dir ? <span className="gitp-file-dir">{dir}</span> : null}
          <span className="gitp-file-name">{name}</span>
        </span>
      </button>

      <span className="gitp-file-actions">
        <button
          type="button"
          className="gitp-icon-btn"
          disabled={disabled}
          aria-label={selected ? `Hide diff for ${path}` : `Show diff for ${path}`}
          title="Show diff"
          onClick={openDiff}
        >
          <FileDiff size={14} aria-hidden="true" />
        </button>

        {onDiscard ? (
          <button
            type="button"
            className="gitp-icon-btn gitp-icon-btn-danger"
            disabled={disabled}
            aria-label={`Discard changes in ${path}`}
            title="Discard changes"
            onClick={() => onDiscard(path)}
          >
            <Undo2 size={14} aria-hidden="true" />
          </button>
        ) : null}

        <button
          type="button"
          className="gitp-icon-btn"
          disabled={disabled}
          aria-label={staged ? `Unstage ${path}` : `Stage ${path}`}
          title={staged ? "Unstage" : "Stage"}
          onClick={() => (staged ? onUnstage(path) : onStage(path))}
        >
          {staged ? <Minus size={14} aria-hidden="true" /> : <Plus size={14} aria-hidden="true" />}
        </button>
      </span>
    </div>
  );
}
