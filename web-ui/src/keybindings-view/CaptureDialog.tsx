import { useCallback, useEffect, useState } from "react";
import { displayChord, formatChord, stepFromEvent } from "../keybindings/chord";
import { reservedOwner } from "../keybindings/host";
import { isPrefixOf } from "../keybindings/chord";
import type { Keymap } from "../keybindings/matcher";
import type { ChordStep, CommandDef, Host } from "../keybindings/types";
import { conflictsFor } from "./rows";

/**
 * Records a chord and reports what it would collide with before anything is
 * written. The three checks are the ones the conflict validator runs at build
 * time, moved to the moment of authoring so a user gets the same answer.
 */

export interface CaptureDialogProps {
  readonly command: CommandDef;
  readonly previous?: string;
  readonly keymap: Keymap;
  readonly host: Host;
  readonly onCommit: (chord: string) => void;
  readonly onCancel: () => void;
}

/** Keys that only modify — recording one alone would capture an empty chord. */
const MODIFIERS = new Set(["Control", "Shift", "Alt", "Meta", "CapsLock"]);

interface Warning {
  readonly kind: "conflict" | "reserved" | "prefix";
  readonly text: string;
}

function checkChord(
  steps: readonly ChordStep[],
  keymap: Keymap,
  host: Host,
  command: CommandDef,
): Warning[] {
  if (steps.length === 0) return [];
  const warnings: Warning[] = [];
  const chordId = formatChord(steps);

  for (const step of steps) {
    const owner = reservedOwner(host, formatChord([step]));
    if (owner) {
      warnings.push({
        kind: "reserved",
        text: `${displayChord([step], host.platform)} is taken by ${owner} and will never reach the app.`,
      });
    }
  }

  for (const clash of conflictsFor(keymap, chordId, command.when, command.id)) {
    warnings.push({
      kind: "conflict",
      text: `Already bound to ${clash.command} in the same scope.`,
    });
  }

  const prefixOf = keymap.all.find((binding) => isPrefixOf(steps, binding.seq));
  if (prefixOf) {
    warnings.push({
      kind: "prefix",
      text: `${displayChord(steps, host.platform)} is the start of ${displayChord(prefixOf.seq, host.platform)}, so it cannot also run a command.`,
    });
  }

  const startsWith = keymap.all.find(
    (binding) => binding.command !== command.id && isPrefixOf(binding.seq, steps),
  );
  if (startsWith) {
    warnings.push({
      kind: "prefix",
      text: `${displayChord(startsWith.seq, host.platform)} already runs ${startsWith.command}, so this chord would never complete.`,
    });
  }

  return warnings;
}

export function CaptureDialog({
  command,
  previous,
  keymap,
  host,
  onCommit,
  onCancel,
}: CaptureDialogProps) {
  const [steps, setSteps] = useState<readonly ChordStep[]>([]);

  const commit = useCallback(() => {
    if (steps.length > 0) onCommit(formatChord(steps));
  }, [steps, onCommit]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      // Everything is captured, including Escape and Enter, so that they can be
      // bound too. The dialog is dismissed with its buttons instead.
      event.preventDefault();
      event.stopPropagation();

      if (MODIFIERS.has(event.key)) return;
      if (event.key === "Backspace" && !event.ctrlKey && !event.metaKey) {
        setSteps((current) => current.slice(0, -1));
        return;
      }
      setSteps((current) => [...current, stepFromEvent(event)]);
    }

    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, []);

  const warnings = checkChord(steps, keymap, host, command);
  const blocking = warnings.some((w) => w.kind === "reserved" || w.kind === "prefix");

  return (
    <div className="kbv-capture-backdrop modal-backdrop" role="presentation">
      <div className="kbv-capture modal-dialog-surface" role="dialog" aria-label="Record keybinding">
        <h2 className="kbv-capture-title">{command.title}</h2>
        <p className="kbv-capture-sub">
          Press the keys you want. Backspace removes the last step.
        </p>

        <div className="kbv-capture-field" aria-live="polite">
          {steps.length === 0 ? (
            <span className="kbv-capture-empty">Waiting for a key…</span>
          ) : (
            steps.map((step, index) => (
              <kbd className="kbv-chip" key={`${index}-${step.key}`}>
                {displayChord([step], host.platform)}
              </kbd>
            ))
          )}
        </div>

        {previous ? <p className="kbv-capture-prev">Replaces {previous}</p> : null}

        <ul className="kbv-capture-warnings">
          {warnings.map((warning) => (
            <li className={`kbv-warning is-${warning.kind}`} key={warning.text}>
              {warning.text}
            </li>
          ))}
        </ul>

        <div className="kbv-capture-actions">
          <button type="button" className="kbv-btn" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="kbv-btn is-primary"
            onClick={commit}
            disabled={steps.length === 0 || blocking}
          >
            {blocking ? "Unusable chord" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
