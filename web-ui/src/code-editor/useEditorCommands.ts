import { useCommands } from "../keybindings/useCommand";

/**
 * Registers the editor and explorer commands.
 *
 * Replaces the panel's hand-rolled Cmd+S listener: Save is now a command like
 * every other, so it appears in the palette, shows up in the keybindings view,
 * and can be rebound — none of which a bare `window.addEventListener` allowed.
 *
 * The explorer's create and upload commands live in the layout instead, because
 * they open an inline field rather than calling an API directly.
 */

export interface EditorCommandDeps {
  readonly hasOpenFile: boolean;
  readonly save: () => void;
  readonly revert: () => void;
  readonly format: () => void;
  readonly goToDefinition: () => void;
  readonly hover: () => void;
  readonly reloadRoot: () => void;
}

export function useEditorCommands(deps: EditorCommandDeps): void {
  // Guarded rather than unregistered: a command that exists but declines to run
  // keeps its row in the palette, which is how a user discovers why it is inert.
  const withFile = (run: () => void) => () => {
    if (deps.hasOpenFile) run();
  };

  useCommands({
    "editor.save": withFile(deps.save),
    "editor.revert": withFile(deps.revert),
    "lsp.format": withFile(deps.format),
    "lsp.goToDefinition": withFile(deps.goToDefinition),
    "lsp.hover": withFile(deps.hover),
    "explorer.reload": deps.reloadRoot,
  });
}
