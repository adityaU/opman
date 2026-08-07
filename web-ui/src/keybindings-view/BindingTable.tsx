import type { Host } from "../keybindings/types";
import type { BindingRow } from "./rows";

/**
 * The bindings table.
 *
 * Rows are plain buttons rather than a grid widget: the list is long, every row
 * has the same three actions, and a native focus order is what makes the view
 * usable from the keyboard it is there to configure.
 */

export interface BindingTableProps {
  readonly rows: readonly BindingRow[];
  readonly host: Host;
  readonly onEdit: (row: BindingRow) => void;
  readonly onUnbind: (row: BindingRow) => void;
  readonly onReset: (row: BindingRow) => void;
}

const SOURCE_LABEL: Readonly<Record<string, string>> = {
  base: "Default",
  platform: "Platform",
  target: "Target",
  host: "Host",
  config: "Config",
  user: "User",
  unbound: "Unbound",
};

export function BindingTable({ rows, host, onEdit, onUnbind, onReset }: BindingTableProps) {
  if (rows.length === 0) {
    return <p className="kbv-empty">No commands match.</p>;
  }

  return (
    <ul className="kbv-rows">
      {rows.map((row) => {
        const modified = row.source === "config" || row.source === "user";
        return (
          <li className="kbv-row" key={row.id}>
            <button
              type="button"
              className="kbv-row-main"
              onClick={() => onEdit(row)}
              title="Change keybinding"
            >
              <span className="kbv-row-title">{row.command.title}</span>
              <code className="kbv-row-id">{row.command.id}</code>
            </button>

            <span className="kbv-row-chord">
              {row.chord ? (
                <kbd className="kbv-chip">{row.chord}</kbd>
              ) : (
                <span className="kbv-row-unbound">unbound</span>
              )}
            </span>

            <code className="kbv-row-when">{row.binding?.when ?? ""}</code>

            <span className={`kbv-badge is-${row.source}`}>{SOURCE_LABEL[row.source]}</span>

            <span className="kbv-row-actions">
              {row.binding ? (
                <button
                  type="button"
                  className="kbv-icon-btn"
                  onClick={() => onUnbind(row)}
                  title="Remove this keybinding"
                >
                  Remove
                </button>
              ) : null}
              {modified ? (
                <button
                  type="button"
                  className="kbv-icon-btn"
                  onClick={() => onReset(row)}
                  title="Reset to the default keybinding"
                >
                  Reset
                </button>
              ) : null}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
