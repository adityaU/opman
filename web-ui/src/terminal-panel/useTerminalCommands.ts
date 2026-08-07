import { useCommands } from "../keybindings/useCommand";
import type { PtyKind, TabInfo } from "./types";

/**
 * Registers the terminal panel's commands.
 *
 * The panel says what it can do; the keymap decides which key does it. Tab
 * cycling wraps, because a keyboard user reaching the last tab expects to come
 * back round rather than to stop.
 */

export interface TerminalCommandDeps {
  readonly tabs: readonly TabInfo[];
  readonly activeTabId: string | null;
  readonly setActiveTabId: (id: string) => void;
  readonly createTab: (kind: PtyKind) => void;
  readonly closeTab: (id: string) => void;
  readonly startRename: (id: string) => void;
  readonly openKindMenu: () => void;
  readonly expand: () => void;
}

function step(
  tabs: readonly TabInfo[],
  activeTabId: string | null,
  delta: number,
): string | undefined {
  if (tabs.length === 0) return undefined;
  const index = tabs.findIndex((tab) => tab.id === activeTabId);
  if (index < 0) return tabs[0]?.id;
  const next = (index + delta + tabs.length) % tabs.length;
  return tabs[next]?.id;
}

export function useTerminalCommands(deps: TerminalCommandDeps): void {
  const move = (delta: number) => () => {
    const id = step(deps.tabs, deps.activeTabId, delta);
    if (id) deps.setActiveTabId(id);
  };

  const withActive = (run: (id: string) => void) => () => {
    if (deps.activeTabId) run(deps.activeTabId);
  };

  useCommands({
    // The plain "new terminal" command opens a shell; the kind menu is its own
    // command, so the common case stays one keystroke.
    "terminal.newTab": () => deps.createTab("shell"),
    "terminal.newTabOfKind": deps.openKindMenu,
    "terminal.nextTab": move(1),
    "terminal.previousTab": move(-1),
    "terminal.closeTab": withActive(deps.closeTab),
    "terminal.renameTab": withActive(deps.startRename),
    "terminal.selectTab": deps.expand,
  });
}
