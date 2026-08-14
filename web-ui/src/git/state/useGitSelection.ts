import { useCallback, useEffect, useMemo, useState } from "react";
import type { GitFileEntry } from "../types";

/**
 * Keyboard selection for the changes list.
 *
 * The three sections are one ordered list as far as the keyboard is concerned,
 * so `j` walks from the last staged file into the first unstaged one without the
 * user having to think about section boundaries.
 *
 * Selection is held by path rather than by index. Staging a file moves it
 * between sections, and an index would silently land the user on whatever slid
 * into that slot — most likely staging the wrong file next.
 */

export type GitSectionVariant = "staged" | "unstaged" | "untracked";

export interface GitSelectionEntry {
  readonly path: string;
  readonly variant: GitSectionVariant;
}

export interface GitSelection {
  readonly entries: readonly GitSelectionEntry[];
  readonly selected: GitSelectionEntry | undefined;
  readonly isSelected: (path: string, variant: GitSectionVariant) => boolean;
  select: (path: string, variant: GitSectionVariant) => void;
  moveDown: () => void;
  moveUp: () => void;
}

function flatten(
  staged: readonly GitFileEntry[],
  unstaged: readonly GitFileEntry[],
  untracked: readonly GitFileEntry[],
): GitSelectionEntry[] {
  return [
    ...staged.map((file) => ({ path: file.path, variant: "staged" as const })),
    ...unstaged.map((file) => ({ path: file.path, variant: "unstaged" as const })),
    ...untracked.map((file) => ({ path: file.path, variant: "untracked" as const })),
  ];
}

export function useGitSelection(
  staged: readonly GitFileEntry[],
  unstaged: readonly GitFileEntry[],
  untracked: readonly GitFileEntry[],
): GitSelection {
  const entries = useMemo(
    () => flatten(staged, unstaged, untracked),
    [staged, unstaged, untracked],
  );
  const [selectedPath, setSelectedPath] = useState<string>();

  const selected = useMemo(
    () => entries.find((entry) => entry.path === selectedPath),
    [entries, selectedPath],
  );

  /**
   * Keep a selection alive across refreshes. A staged file keeps its path and
   * simply changes section, so it stays selected; one that disappears entirely
   * — committed, discarded — hands the selection to the first remaining row
   * rather than leaving the keyboard with nothing to act on.
   */
  useEffect(() => {
    if (entries.length === 0) {
      setSelectedPath(undefined);
      return;
    }
    if (selectedPath && entries.some((entry) => entry.path === selectedPath)) return;
    setSelectedPath(entries[0].path);
  }, [entries, selectedPath]);

  const step = useCallback(
    (delta: number) => {
      if (entries.length === 0) return;
      const index = entries.findIndex((entry) => entry.path === selectedPath);
      if (index < 0) {
        setSelectedPath(entries[0].path);
        return;
      }
      // Clamped, not wrapped: a long changes list is scanned, and wrapping from
      // the last file back to the first reads as a jump rather than a move.
      const next = Math.min(Math.max(index + delta, 0), entries.length - 1);
      setSelectedPath(entries[next].path);
    },
    [entries, selectedPath],
  );

  return {
    entries,
    selected,
    isSelected: useCallback(
      (path, variant) => selected?.path === path && selected.variant === variant,
      [selected],
    ),
    select: useCallback((path) => setSelectedPath(path), []),
    moveDown: useCallback(() => step(1), [step]),
    moveUp: useCallback(() => step(-1), [step]),
  };
}
