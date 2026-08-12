import type { CmdlineModel } from "../state/cmdline";

export interface CmdlineProps {
  readonly state: CmdlineModel;
}

function text(cells: CmdlineModel["content"]): string {
  return cells.map((cell) => cell.text).join("");
}

function commandLine(content: string, position: number): React.ReactElement {
  const pivot = Math.max(0, Math.min(position, content.length));
  return (
    <span className="nvim-cmdline-content">
      <span>{content.slice(0, pivot)}</span>
      <span className="nvim-cmdline-cursor" aria-hidden="true" />
      <span>{content.slice(pivot)}</span>
    </span>
  );
}

export function Cmdline({ state }: CmdlineProps): React.ReactElement | null {
  if (!state.visible && state.block.length === 0) return null;
  return (
    <section className="nvim-cmdline-overlay" aria-live="polite" aria-label="Neovim command line">
      {state.block.length > 0 && (
        <div className="nvim-cmdline-block" aria-label="Command block">
          {state.block.map((line, index) => <div key={index} style={{ paddingInlineStart: `${state.indent}ch` }}>{text(line)}</div>)}
        </div>
      )}
      {state.visible && (
        <div className="nvim-cmdline-line" data-position={state.position}>
          {state.firstChar && <span className="nvim-cmdline-firstc" aria-label={`Command prefix ${state.firstChar}`}>{state.firstChar}</span>}
          {state.prompt && <span className="nvim-cmdline-prompt">{state.prompt}</span>}
          {commandLine(text(state.content), state.position)}
          {state.specialChar && <span className="nvim-cmdline-special" aria-label={`Special key ${state.specialChar}`}>{state.specialShift ? "Shift+" : ""}{state.specialChar}</span>}
        </div>
      )}
    </section>
  );
}

export default Cmdline;
