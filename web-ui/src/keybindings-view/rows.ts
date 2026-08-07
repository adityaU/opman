import { displayChord } from "../keybindings/chord";
import { COMMANDS, CATEGORY_ORDER } from "../keybindings/commands";
import type { Keymap } from "../keybindings/matcher";
import type { BindingSource, CommandDef, Host, ResolvedBinding } from "../keybindings/types";

/**
 * Rows for the keybindings table.
 *
 * One row per (command, chord), plus one row per command that has no chord at
 * all — an unbound command is the thing a user most often comes here to fix, so
 * it must be visible rather than absent.
 */

export interface BindingRow {
  readonly id: string;
  readonly command: CommandDef;
  readonly binding?: ResolvedBinding;
  readonly chord: string;
  readonly source: BindingSource | "unbound";
}

export interface RowFilters {
  readonly query: string;
  /** A recorded chord id; when set, `query` is ignored. */
  readonly chordId?: string;
  readonly source?: BindingSource;
  readonly onlyUnbound?: boolean;
  readonly onlyModified?: boolean;
}

function categoryRank(category: string): number {
  const index = CATEGORY_ORDER.indexOf(category);
  return index < 0 ? CATEGORY_ORDER.length : index;
}

export function buildRows(keymap: Keymap, host: Host): BindingRow[] {
  const rows: BindingRow[] = [];

  for (const command of COMMANDS) {
    const bindings = keymap.chordsFor(command.id);
    if (bindings.length === 0) {
      rows.push({ id: command.id, command, chord: "", source: "unbound" });
      continue;
    }
    for (const binding of bindings) {
      rows.push({
        id: `${command.id}::${binding.id}`,
        command,
        binding,
        chord: displayChord(binding.seq, host.platform),
        source: binding.source,
      });
    }
  }

  return rows.sort((a, b) => {
    const byCategory = categoryRank(a.command.category) - categoryRank(b.command.category);
    if (byCategory !== 0) return byCategory;
    return a.command.title.localeCompare(b.command.title);
  });
}

/** Case-insensitive match against title, id, category and rendered chord. */
function matchesQuery(row: BindingRow, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (needle.length === 0) return true;
  return (
    row.command.title.toLowerCase().includes(needle) ||
    row.command.id.toLowerCase().includes(needle) ||
    row.command.category.toLowerCase().includes(needle) ||
    row.chord.toLowerCase().includes(needle) ||
    (row.binding?.when ?? "").toLowerCase().includes(needle)
  );
}

export function filterRows(rows: readonly BindingRow[], filters: RowFilters): BindingRow[] {
  return rows.filter((row) => {
    if (filters.onlyUnbound && row.source !== "unbound") return false;
    if (filters.onlyModified && row.source !== "config" && row.source !== "user") return false;
    if (filters.source && row.source !== filters.source) return false;
    // A recorded chord answers "what is this key bound to", so it replaces the
    // text query rather than narrowing it.
    if (filters.chordId) return row.binding?.id === filters.chordId;
    return matchesQuery(row, filters.query);
  });
}

/** Everything else bound to the same chord in an overlapping scope. */
export function conflictsFor(
  keymap: Keymap,
  chordId: string,
  when: string | undefined,
  ignoreCommand?: string,
): ResolvedBinding[] {
  return keymap.all.filter(
    (binding) =>
      binding.id === chordId &&
      binding.command !== ignoreCommand &&
      binding.when === when,
  );
}
