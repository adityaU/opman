import { useCommands } from "../keybindings/useCommand";
import type { PtyKind } from "./types";

/**
 * Registers the terminal panel's commands.
 *
 * The panel says what it can do; the keymap decides which key does it. The
 * vocabulary is shells rather than tabs: there is one terminal per pane, and
 * the shells it can show are shared with every other pane. So "next" moves this
 * terminal to the project's next shell rather than to a tab of its own, and
 * "kill" ends a program rather than closing a view.
 */

export interface TerminalCommandDeps {
  readonly hasShell: boolean;
  readonly newShell: (kind: PtyKind) => void;
  /** Absent when no shell is being shown, so the command is a no-op. */
  readonly killShell?: () => void;
  readonly selectShell: () => void;
  readonly step: (delta: number) => void;
  readonly clear: () => void;
  readonly expand: () => void;
  readonly find: () => void;
}

export function useTerminalCommands(deps: TerminalCommandDeps): void {
  const whenShown = (run: () => void) => () => {
    if (deps.hasShell) run();
  };

  useCommands({
    // The plain "new terminal" command opens a shell; the other kinds go
    // through the picker, so the common case stays one keystroke.
    "terminal.newShell": () => deps.newShell("shell"),
    "terminal.nextShell": () => deps.step(1),
    "terminal.previousShell": () => deps.step(-1),
    "terminal.selectShell": deps.selectShell,
    "terminal.killShell": () => deps.killShell?.(),
    // Renaming happens in the picker, so the command opens it.
    "terminal.renameShell": deps.selectShell,
    "terminal.clear": whenShown(deps.clear),
    "terminal.expand": deps.expand,
    "terminal.find": deps.find,
  });
}
