import type { NvimState } from "../state/store";

export interface StatuslineProps {
  readonly state: NvimState;
  readonly connection: string;
}

function cellsToText(cells: readonly { readonly text: string }[]): string {
  return cells.map((cell) => cell.text).join("");
}

function modeLabel(mode: string): string {
  return mode.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function Statusline({ state, connection }: StatuslineProps): React.ReactElement {
  const mode = state.modes.current;
  const modeInfo = state.modes.modes.get(mode);
  const name = modeInfo?.name || modeLabel(mode);
  const shortName = modeInfo?.shortName || mode.slice(0, 3).toUpperCase();
  const showmode = cellsToText(state.messages.mode);
  const showcmd = cellsToText(state.messages.command);
  const ruler = cellsToText(state.messages.ruler);
  return (
    <footer className="nvim-statusline-overlay" aria-label="Neovim status">
      <span className="nvim-statusline-mode" data-mode={mode}>
        <span className="nvim-statusline-mode-short" aria-hidden="true">{shortName}</span>
        <span>{name}</span>
      </span>
      {showmode && <span className="nvim-statusline-showmode">{showmode}</span>}
      {showcmd && <span className="nvim-statusline-showcmd" aria-label="Pending keys">{showcmd}</span>}
      {state.options.title && <span className="nvim-statusline-title">{state.options.title}</span>}
      <span className="nvim-statusline-connection" data-connection={connection}>{connection}</span>
      {ruler && <span className="nvim-statusline-ruler">{ruler}</span>}
    </footer>
  );
}

export default Statusline;
