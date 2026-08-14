import React, { useCallback, useMemo, useState } from "react";
import { Pencil, Plus, SquareTerminal, Trash2 } from "lucide-react";
import { ALL_PTY_KINDS, KIND_LABELS, type PtyKind, type PtySession } from "./types";

/**
 * Which shell this terminal shows.
 *
 * Shown inside the pane rather than as a dialog: the pane is already the
 * question ("what goes here"), and a modal over it would make choosing a shell
 * feel heavier than opening a new one. The list is of *running* shells, so what
 * it offers is exactly what exists — including shells another pane or an agent
 * started.
 */

export interface ShellPickerProps {
  readonly shells: readonly PtySession[];
  readonly loading: boolean;
  /** Name of the project these shells belong to, for the heading. */
  readonly projectName: string;
  readonly onPick: (ptyId: string) => void;
  readonly onCreate: (kind: PtyKind) => void;
  readonly onKill: (ptyId: string) => void;
  readonly onRename: (ptyId: string, label: string) => void;
  /** Shown when the picker is a switch rather than a first choice. */
  readonly onCancel?: () => void;
}

export const ShellPicker: React.FC<ShellPickerProps> = function ShellPicker({
  shells,
  loading,
  projectName,
  onPick,
  onCreate,
  onKill,
  onRename,
  onCancel,
}) {
  const [kindsOpen, setKindsOpen] = useState(false);
  const [renaming, setRenaming] = useState<{ id: string; value: string } | null>(null);

  const commitRename = useCallback(() => {
    if (renaming && renaming.value.trim()) onRename(renaming.id, renaming.value.trim());
    setRenaming(null);
  }, [onRename, renaming]);

  // Busy shells first: the one running a build is the one being looked for.
  const ordered = useMemo(
    () =>
      [...shells].sort((a, b) => {
        if (a.activity !== b.activity) return a.activity === "running" ? -1 : 1;
        return a.label.localeCompare(b.label, undefined, { numeric: true });
      }),
    [shells],
  );

  const newShell = useCallback(
    (kind: PtyKind) => {
      setKindsOpen(false);
      onCreate(kind);
    },
    [onCreate],
  );

  return (
    <div className="tsp" data-surface="terminal">
      <div className="tsp-head">
        <SquareTerminal size={14} aria-hidden="true" />
        <span className="tsp-title">Shells in {projectName}</span>
        {onCancel && (
          <button type="button" className="tsp-cancel" onClick={onCancel}>
            Cancel
          </button>
        )}
      </div>

      <ul className="tsp-list" role="listbox" aria-label={`Shells in ${projectName}`}>
        {ordered.map((shell) =>
          renaming?.id === shell.id ? (
            <li key={shell.id}>
              <input
                className="tsp-rename"
                value={renaming.value}
                aria-label={`Rename ${shell.label}`}
                autoFocus
                onChange={(event) => setRenaming({ id: shell.id, value: event.target.value })}
                onBlur={commitRename}
                onKeyDown={(event) => {
                  if (event.key === "Enter") commitRename();
                  if (event.key === "Escape") setRenaming(null);
                }}
              />
            </li>
          ) : (
            <li key={shell.id}>
              <button
                type="button"
                role="option"
                aria-selected={false}
                className="tsp-row"
                onClick={() => onPick(shell.id)}
              >
                <span
                  className={`tsp-dot${shell.activity === "running" ? " is-running" : ""}`}
                  aria-hidden="true"
                />
                <span className="tsp-label">{shell.label}</span>
                <span className="tsp-kind">{KIND_LABELS[shell.kind]}</span>
                {shell.activity === "running" && <span className="tsp-busy">running</span>}
              </button>
              {/* Rename and kill sit outside the row's own click target, so
                  reaching for a shell can never end or edit one. */}
              <button
                type="button"
                className="tsp-edit"
                aria-label={`Rename ${shell.label}`}
                title={`Rename ${shell.label}`}
                onClick={() => setRenaming({ id: shell.id, value: shell.label })}
              >
                <Pencil size={12} />
              </button>
              <button
                type="button"
                className="tsp-kill"
                aria-label={`Kill ${shell.label}`}
                title={`Kill ${shell.label}`}
                onClick={() => onKill(shell.id)}
              >
                <Trash2 size={12} />
              </button>
            </li>
          ),
        )}
        {ordered.length === 0 && (
          <li className="tsp-empty">
            {loading ? "Looking for running shells…" : "No shells running here yet"}
          </li>
        )}
      </ul>

      <div className="tsp-new">
        <button type="button" className="tsp-new-btn" onClick={() => newShell("shell")}>
          <Plus size={13} aria-hidden="true" />
          <span>New shell</span>
        </button>
        {/* The other kinds are rare enough to be worth one extra click, so the
            common case stays a single button rather than a menu. */}
        <button
          type="button"
          className="tsp-new-more"
          aria-expanded={kindsOpen}
          onClick={() => setKindsOpen((open) => !open)}
        >
          {kindsOpen ? "Less" : "Other…"}
        </button>
      </div>

      {kindsOpen && (
        <div className="tsp-kinds" role="group" aria-label="Other terminal kinds">
          {ALL_PTY_KINDS.filter((kind) => kind !== "shell").map((kind) => (
            <button key={kind} type="button" className="tsp-kind-btn" onClick={() => newShell(kind)}>
              {KIND_LABELS[kind]}
            </button>
          ))}
        </div>
      )}
    </div>
  );
};
