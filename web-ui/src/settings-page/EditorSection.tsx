import { Code2, Terminal } from "lucide-react";
import { useIsMobile } from "../hooks/useIsMobile";
import type { EditorEngine } from "../editor-engine/preference";

interface EditorOption {
  readonly engine: EditorEngine;
  readonly label: string;
  readonly description: string;
  readonly Icon: typeof Code2;
}

const OPTIONS: readonly EditorOption[] = [
  {
    engine: "codemirror",
    label: "CodeMirror",
    description: "Plain editing with familiar text-editor shortcuts.",
    Icon: Code2,
  },
  {
    engine: "neovim",
    label: "Neovim",
    description: "Neovim motions, operators, and registers drive the same editor.",
    Icon: Terminal,
  },
];

export interface EditorSectionProps {
  readonly engine: EditorEngine;
  readonly onEngineChange: (engine: EditorEngine) => void;
}

export function EditorSection({ engine, onEngineChange }: EditorSectionProps) {
  const isMobile = useIsMobile();
  const neovimUnavailable = isMobile;
  const tabStopEngine = engine === "neovim" && neovimUnavailable ? "codemirror" : engine;

  const selectWithKeyboard = (index: number, event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== "ArrowRight" && event.key !== "ArrowDown"
      && event.key !== "ArrowLeft" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const direction = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : -1;
    const nextIndex = (index + direction + OPTIONS.length) % OPTIONS.length;
    const next = OPTIONS[nextIndex];
    if (next.engine === "neovim" && neovimUnavailable) return;
    onEngineChange(next.engine);
  };

  return (
    <div className="stg-stack">
      <section className="stg-card">
        <h3 className="stg-card-title">Editing engine</h3>
        <p className="stg-card-note">
          Choose how code files behave on this device. This choice stays in this browser;
          it does not follow your account to another laptop.
        </p>
        <div className="stg-editor-options" role="radiogroup" aria-label="Editing engine">
          {OPTIONS.map(({ engine: optionEngine, label, description, Icon }, index) => {
            const disabled = optionEngine === "neovim" && neovimUnavailable;
            const selected = engine === optionEngine;
            return (
              <button
                key={optionEngine}
                type="button"
                role="radio"
                className={`stg-editor-option${selected ? " is-selected" : ""}`}
                aria-checked={selected}
                aria-describedby={disabled ? "stg-editor-mobile-note" : undefined}
                disabled={disabled}
                tabIndex={optionEngine === tabStopEngine ? 0 : -1}
                onClick={() => onEngineChange(optionEngine)}
                onKeyDown={(event) => selectWithKeyboard(index, event)}
              >
                <span className="stg-editor-option-icon" aria-hidden="true">
                  <Icon size={17} />
                </span>
                <span className="stg-editor-option-copy">
                  <span className="stg-editor-option-label">{label}</span>
                  <span className="stg-editor-option-description">{description}</span>
                </span>
                <span className="stg-editor-radio" aria-hidden="true" />
              </button>
            );
          })}
        </div>
        {isMobile && (
          <p id="stg-editor-mobile-note" className="stg-editor-unavailable" role="status">
            Neovim is desktop-only, so mobile uses CodeMirror while this preference is kept
            for your next desktop session.
          </p>
        )}
      </section>
    </div>
  );
}
