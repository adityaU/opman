import { moveFocus } from "./navRegions";
import { useCommands } from "./useCommand";

/**
 * Wires `nav.focus*` to the shell registry.
 *
 * Mounted once, beside the keymap listener, because the commands belong to the
 * shell rather than to any surface inside it — the whole point is that they
 * work the same whether focus is in the sidebar, in a pane or on the rail.
 */
export function useDirectionalNav(): void {
  useCommands({
    "nav.focusLeft": () => moveFocus("left"),
    "nav.focusRight": () => moveFocus("right"),
    "nav.focusUp": () => moveFocus("up"),
    "nav.focusDown": () => moveFocus("down"),
  });
}
