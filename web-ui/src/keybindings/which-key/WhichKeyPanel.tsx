import { useEffect, useMemo, useState } from "react";
import { displayStep } from "../chord";
import { commandLabel, findCommand } from "../commands";
import { useKeymapContext } from "../KeymapContext";
import { buildHints } from "../matcher";
import type { HintGroup, MatchContext } from "../matcher";
import type { ResolvedBinding } from "../types";
import type { KeymapListenerState } from "../useKeymapListener";
import { whenEvaluator } from "../when";

/**
 * The which-key hint panel.
 *
 * Appears only once a chord has been pending longer than `delayMs`, so a
 * fluent `Space g m` never draws anything and a hesitant one is answered. The
 * hints come from the composed keymap, which means rebinding a key in the
 * keybindings view changes this panel on the next keypress with no other work.
 */

export interface WhichKeyPanelProps {
  readonly listener: KeymapListenerState;
}

function labelFor(binding: ResolvedBinding): string {
  if (binding.label) return binding.label;
  const command = findCommand(binding.command);
  return command ? commandLabel(command) : binding.command;
}

function sortEntries(groups: HintGroup[], sortBy: "group" | "key" | "label"): HintGroup[] {
  if (sortBy === "group") return groups;
  return groups.map((group) => ({
    ...group,
    entries: [...group.entries].sort((a, b) =>
      sortBy === "key" ? a.key.key.localeCompare(b.key.key) : a.label.localeCompare(b.label),
    ),
  }));
}

export function WhichKeyPanel({ listener }: WhichKeyPanelProps) {
  const { keymap, mode, config, context, host } = useKeymapContext();
  const { pending, revealed } = listener;
  const [elapsed, setElapsed] = useState(false);

  const delay = config.whichKey.delayMs;

  useEffect(() => {
    if (!pending) {
      setElapsed(false);
      return undefined;
    }
    // The timer restarts per chord level, so descending a namespace does not
    // make the panel flash away and back.
    setElapsed(false);
    const timer = setTimeout(() => setElapsed(true), delay);
    return () => clearTimeout(timer);
  }, [pending, delay]);

  const groups = useMemo(() => {
    if (!pending) return [];
    const matchContext: MatchContext = {
      mode,
      textInput: false,
      isTrue: whenEvaluator(context),
    };
    return sortEntries(
      buildHints(keymap, pending.steps, matchContext, labelFor),
      config.whichKey.sortBy,
    );
  }, [pending, keymap, mode, context, config.whichKey.sortBy]);

  if (!config.whichKey.enabled) return null;
  if (!pending || (!elapsed && !revealed)) return null;
  if (groups.length === 0) return null;

  const breadcrumb = pending.steps.map((step) => displayStep(step, host.platform, mode)).join(" ");

  return (
    <div
      className="which-key modal-popover-surface"
      role="dialog"
      aria-label="Keybinding hints"
      aria-live="polite"
      tabIndex={-1}
    >
      <div className="which-key-head">
        <span className="which-key-crumb">{breadcrumb}</span>
        <span className="which-key-hint">
          <kbd>esc</kbd> cancel · <kbd>⌫</kbd> back
        </span>
      </div>
      <div className="which-key-groups">
        {groups.map((group) => (
          <section className="which-key-group" key={group.group}>
            <h3 className="which-key-group-name">{group.group}</h3>
            <ul className="which-key-entries">
              {group.entries.map((entry) => (
                <li className="which-key-entry" key={`${group.group}-${entry.label}-${entry.key.key}`}>
                  <kbd className="which-key-key">{displayStep(entry.key, host.platform, mode)}</kbd>
                  <span
                    className={entry.isPrefix ? "which-key-label is-prefix" : "which-key-label"}
                  >
                    {entry.label}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </div>
  );
}

/**
 * The compact strip for normal mode.
 *
 * A Ctrl+K chord has one obvious continuation set and a VSCode user expects a
 * status line, not a panel, so this shows the pending chord and nothing else.
 */
export function PendingChordStrip({ listener }: WhichKeyPanelProps) {
  const { host, mode } = useKeymapContext();
  const { pending } = listener;
  if (!pending || mode === "vim") return null;

  const breadcrumb = pending.steps.map((step) => displayStep(step, host.platform, mode)).join(" ");
  return (
    <div className="which-key-strip" role="status">
      <kbd>{breadcrumb}</kbd>
      <span>was pressed. Waiting for the second key of the chord…</span>
    </div>
  );
}
