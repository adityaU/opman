import React from "react";
import { ChevronUp, Keyboard, CornerDownLeft } from "lucide-react";
import { ARROW_KEYS, EXTRA_KEYS, PRIMARY_KEYS, type KeySpec, type ModifierState } from "./keys";
import type { MobileKeys } from "./useMobileKeys";

/** A key button. Touch targets are 40px minimum — thumbs, not cursors. */
function Key({
  spec,
  onPress,
  className = "",
}: {
  spec: KeySpec;
  onPress: (seq: string) => void;
  className?: string;
}) {
  return (
    <button
      type="button"
      className={`tkb-key ${className}`}
      aria-label={spec.title}
      title={spec.title}
      // Pointer-down, not click: a terminal key should register the instant the
      // thumb lands, and preventDefault keeps the soft keyboard from closing.
      onPointerDown={(e) => {
        e.preventDefault();
        onPress(spec.seq);
      }}
    >
      {spec.label}
    </button>
  );
}

function Modifier({
  label,
  state,
  onToggle,
}: {
  label: string;
  state: ModifierState;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className={`tkb-key tkb-mod tkb-mod-${state}`}
      aria-pressed={state !== "off"}
      aria-label={
        state === "locked" ? `${label} locked` : state === "armed" ? `${label} armed for next key` : label
      }
      onPointerDown={(e) => {
        e.preventDefault();
        onToggle();
      }}
    >
      {label}
      {state === "locked" && <span className="tkb-mod-lock" aria-hidden="true" />}
    </button>
  );
}

/**
 * The on-screen key bar for the touch terminal.
 *
 * Row one is what you always need: escape, tab, the two modifiers and the
 * arrows. Row two is opt-in and remembered, because vertical space on a phone
 * is worth more than convenience the user has not asked for.
 */
export function MobileKeyBar({ keys }: { keys: MobileKeys }) {
  const armed = keys.ctrl !== "off" || keys.alt !== "off";

  return (
    <div className={`tkb${armed ? " tkb-armed" : ""}`} role="group" aria-label="Terminal keys">
      <div className="tkb-row tkb-row-primary">
        {PRIMARY_KEYS.map((spec) => (
          <Key key={spec.id} spec={spec} onPress={keys.press} className="tkb-key-word" />
        ))}
        <Modifier label="ctrl" state={keys.ctrl} onToggle={keys.toggleCtrl} />
        <Modifier label="alt" state={keys.alt} onToggle={keys.toggleAlt} />

        <div className="tkb-arrows">
          {ARROW_KEYS.map((spec) => (
            <Key key={spec.id} spec={spec} onPress={keys.press} className="tkb-key-arrow" />
          ))}
        </div>

        <button
          type="button"
          className="tkb-key tkb-key-icon"
          aria-label="Return"
          onPointerDown={(e) => { e.preventDefault(); keys.press("\r"); }}
        >
          <CornerDownLeft size={15} />
        </button>
        <button
          type="button"
          className="tkb-key tkb-key-icon"
          aria-label="Show keyboard"
          onPointerDown={(e) => { e.preventDefault(); keys.focusTerminal(); }}
        >
          <Keyboard size={15} />
        </button>
        <button
          type="button"
          className={`tkb-key tkb-key-icon tkb-more${keys.extrasOpen ? " open" : ""}`}
          aria-label={keys.extrasOpen ? "Hide more keys" : "Show more keys"}
          aria-expanded={keys.extrasOpen}
          onPointerDown={(e) => { e.preventDefault(); keys.toggleExtras(); }}
        >
          <ChevronUp size={15} />
        </button>
      </div>

      {keys.extrasOpen && (
        <div className="tkb-row tkb-row-extras">
          {EXTRA_KEYS.map((spec) => (
            <Key key={spec.id} spec={spec} onPress={keys.press} className="tkb-key-word" />
          ))}
        </div>
      )}
    </div>
  );
}
