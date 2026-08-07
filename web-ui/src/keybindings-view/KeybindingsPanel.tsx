import React, { useMemo, useState } from "react";
import { formatChord, stepFromEvent } from "../keybindings/chord";
import { useKeymapContext } from "../keybindings/KeymapContext";
import type { CommandDef } from "../keybindings/types";
import { CaptureDialog } from "./CaptureDialog";
import { LeaderTree } from "./LeaderTree";
import { BindingTable } from "./BindingTable";
import { buildRows, filterRows } from "./rows";
import type { RowFilters } from "./rows";
import { useKeybindingsEditor } from "./useKeybindingsEditor";

/**
 * The keybindings editor, without any surrounding chrome.
 *
 * Everything shown is derived from the composed keymap, so the table, the leader tree and
 * the app itself can never disagree about what a key does. Lives apart from any host
 * surface because it is now embedded in the settings page rather than a dialog of its own.
 */

type Tab = "bindings" | "tree";

export function KeybindingsPanel() {
  const { keymap, host, mode } = useKeymapContext();
  const editor = useKeybindingsEditor();
  const [tab, setTab] = useState<Tab>("bindings");
  const [filters, setFilters] = useState<RowFilters>({ query: "" });
  const [recording, setRecording] = useState(false);
  const [editing, setEditing] = useState<{ command: CommandDef; previous?: string }>();

  const rows = useMemo(() => buildRows(keymap, host), [keymap, host]);
  const visible = useMemo(() => filterRows(rows, filters), [rows, filters]);

  /**
   * With recording on, the search field captures a chord instead of text — the "what is
   * this key bound to" question, which no amount of typing answers.
   */
  const onSearchKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (!recording) return;
    event.preventDefault();
    if (event.key === "Escape") {
      setRecording(false);
      setFilters((current) => ({ ...current, chordId: undefined }));
      return;
    }
    setFilters((current) => ({
      ...current,
      chordId: formatChord([stepFromEvent(event.nativeEvent)]),
    }));
  };

  const patch = (next: Partial<RowFilters>) =>
    setFilters((current) => ({ ...current, ...next }));

  return (
    <div className="kbv-panel">
      <div className="kbv-panel-head">
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
