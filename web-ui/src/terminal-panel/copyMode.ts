/**
 * Keyboard-driven copy mode over the scrollback, in the style of tmux and vim.
 *
 * The mode is a cursor on one buffer line and an optional anchor on another. Lines between
 * them are selected. This file is the pure part: given a state and a key, the next state
 * and what to do; the terminal glue applies it.
 */

export interface CopyState {
  /** Buffer row the cursor is on. */
  readonly cursor: number;
  /** Row the selection started from, when one has been started. */
  readonly anchor: number | null;
}

export interface Geometry {
  /** Number of rows in the whole buffer. */
  readonly rows: number;
  /** Rows visible at once, for half-page moves. */
  readonly page: number;
}

export type CopyEffect = "none" | "copy" | "exit";

export interface CopyStep {
  readonly state: CopyState;
  readonly effect: CopyEffect;
}

const clamp = (row: number, g: Geometry) => Math.max(0, Math.min(g.rows - 1, row));

/** The lines the selection covers, first to last. */
export function selectedRange(state: CopyState): [number, number] {
  const from = state.anchor ?? state.cursor;
  return [Math.min(from, state.cursor), Math.max(from, state.cursor)];
}

export function copyModeStep(
  state: CopyState,
  key: string,
  ctrl: boolean,
  g: Geometry,
): CopyStep {
  const move = (row: number): CopyStep => ({
    state: { ...state, cursor: clamp(row, g) },
    effect: "none",
  });
  const half = Math.max(1, Math.floor(g.page / 2));

  if (ctrl) {
    if (key === "u") return move(state.cursor - half);
    if (key === "d") return move(state.cursor + half);
    if (key === "b") return move(state.cursor - g.page);
    if (key === "f") return move(state.cursor + g.page);
    return { state, effect: "none" };
  }
  switch (key) {
    case "j":
    case "ArrowDown":
      return move(state.cursor + 1);
    case "k":
    case "ArrowUp":
      return move(state.cursor - 1);
    case "PageUp":
      return move(state.cursor - g.page);
    case "PageDown":
      return move(state.cursor + g.page);
    case "g":
    case "Home":
      return move(0);
    case "G":
    case "End":
      return move(g.rows - 1);
    case "v":
    case "V":
    case " ":
      return {
        state: { ...state, anchor: state.anchor === null ? state.cursor : null },
        effect: "none",
      };
    case "y":
    case "Enter":
      return { state, effect: "copy" };
    case "q":
    case "Escape":
      return { state, effect: "exit" };
    default:
      return { state, effect: "none" };
  }
}
