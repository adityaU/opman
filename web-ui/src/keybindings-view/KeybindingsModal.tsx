import { useMemo, useState } from "react";
import { formatChord, stepFromEvent } from "../keybindings/chord";
import { useKeymapContext } from "../keybindings/KeymapContext";
import { useEscape } from "../hooks/useKeyboard";
import type { CommandDef } from "../keybindings/types";
import { CaptureDialog } from "./CaptureDialog";
import { LeaderTree } from "./LeaderTree";
import { BindingTable } from "./BindingTable";
import { buildRows, filterRows } from "./rows";
import type { RowFilters } from "./rows";
import { useKeybindingsEditor } from "./useKeybindingsEditor";

/**
 * Preferences: Keyboard Shortcuts.
 *
 * Replaces the read-only cheatsheet. Everything shown is derived from the
 * composed keymap, so the table, the leader tree and the app itself can never
 * disagree about what a key does.
 */

export interface KeybindingsModalProps {
  readonly onClose: () => void;
}

type Tab = "bindings" | "tree";

export function KeybindingsModal({ onClose }: KeybindingsModalProps) {
  const { keymap, host, mode } = useKeymapContext();
  const editor = useKeybindingsEditor();
  const [tab, setTab] = useState<Tab>("bindings");
  const [filters, setFilters] = useState<RowFilters>({ query: "" });
  const [recording, setRecording] = useState(false);
  const [editing, setEditing] = useState<{ command: CommandDef; previous?: string }>();

  useEscape(onClose);

  const rows = useMemo(() => buildRows(keymap, host), [keymap, host]);
  const visible = useMemo(() => filterRows(rows, filters), [rows, filters]);

  /**
   * With recording on, the search field captures a chord instead of text — the
   * "what is this key bound to" question, which no amount of typing answers.
   */
  const onSearchKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (!recording) return;
    event.preventDefault();
    if (event.key === "Escape") {
      setRecording(false);
      setFilters((f) => ({ ...f, chordId: undefined }));
      return;
    }
    setFilters((f) => ({ ...f, chordId: formatChord([stepFromEvent(event.nativeEvent)]) }));
  };

  const patch = (next: Partial<RowFilters>) => setFilters((f) => ({ ...f, ...next }));

  return (
    <div className="kbv-backdrop modal-backdrop" role="presentation" onClick={onClose}>
      <div
        className="kbv modal-dialog-surface"
        role="dialog"
        aria-label="Keyboard shortcuts"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="kbv-head">
          <h2 className="kbv-title">Keyboard Shortcuts</h2>

          <div className="kbv-mode" role="group" aria-label="Keymap mode">
            {(["normal", "vim"] as const).map((option) => (
              <button
                key={option}
                type="button"
                className={mode === option ? "kbv-mode-btn is-active" : "kbv-mode-btn"}
                onClick={() => editor.setMode(option)}
              >
                {option === "normal" ? "Normal" : "Vim"}
              </button>
            ))}
          </div>

          <button type="button" className="kbv-close" onClick={onClose} aria-label="Close">
            ×
          </button>
        </header>

        <div className="kbv-tabs" role="tablist">
          <button
            type="button"
            role="tab"
            aria-selected={tab === "bindings"}
            className={tab === "bindings" ? "kbv-tab is-active" : "kbv-tab"}
            onClick={() => setTab("bindings")}
          >
            All bindings
          </button>
          {mode === "vim" ? (
            <button
              type="button"
              role="tab"
              aria-selected={tab === "tree"}
              className={tab === "tree" ? "kbv-tab is-active" : "kbv-tab"}
              onClick={() => setTab("tree")}
            >
              Leader tree
            </button>
          ) : null}
        </div>

        {tab === "bindings" ? (
          <>
            <div className="kbv-controls">
              <input
                className="kbv-search"
                type="text"
                placeholder={recording ? "Press a key…" : "Search commands and keys"}
                value={filters.chordId ?? filters.query}
                readOnly={recording}
                onKeyDown={onSearchKeyDown}
                onChange={(event) => patch({ query: event.target.value, chordId: undefined })}
              />
              <button
                type="button"
                className={recording ? "kbv-btn is-active" : "kbv-btn"}
                onClick={() => {
                  setRecording((on) => !on);
                  patch({ chordId: undefined });
                }}
              >
                Record keys
              </button>
              <label className="kbv-check">
                <input
                  type="checkbox"
                  checked={filters.onlyUnbound ?? false}
                  onChange={(event) => patch({ onlyUnbound: event.target.checked })}
                />
                Unbound
              </label>
              <label className="kbv-check">
                <input
                  type="checkbox"
                  checked={filters.onlyModified ?? false}
                  onChange={(event) => patch({ onlyModified: event.target.checked })}
                />
                Modified
              </label>
            </div>

            <BindingTable
              rows={visible}
              host={host}
              onEdit={(row) => setEditing({ command: row.command, previous: row.chord })}
              onUnbind={(row) => row.binding && editor.unbind(row.command.id, row.binding.id)}
              onReset={(row) => editor.reset(row.command.id)}
            />
          </>
        ) : (
          <LeaderTree keymap={keymap} host={host} leaderLabel={editor.config.leader} />
        )}

        <footer className="kbv-foot">
          <span className="kbv-foot-path">{editor.path ?? "keybindings.json"}</span>
          {editor.saving ? <span className="kbv-foot-state">Saving…</span> : null}
          {editor.error ? <span className="kbv-foot-error">{editor.error}</span> : null}
          {editor.diagnostics.map((diagnostic) => (
            <span className="kbv-foot-error" key={diagnostic.message}>
              {diagnostic.line ? `line ${diagnostic.line}: ` : ""}
              {diagnostic.message}
            </span>
          ))}
          <button type="button" className="kbv-btn" onClick={editor.resetAll}>
            Reset all
          </button>
        </footer>
      </div>

      {editing ? (
        <CaptureDialog
          command={editing.command}
          previous={editing.previous}
          keymap={keymap}
          host={host}
          onCancel={() => setEditing(undefined)}
          onCommit={(chord) => {
            const previous = keymap
              .chordsFor(editing.command.id)
              .find((binding) => binding.id !== chord)?.id;
            editor.rebind(editing.command.id, chord, previous, editing.command.when);
            setEditing(undefined);
          }}
        />
      ) : null}
    </div>
  );
}
