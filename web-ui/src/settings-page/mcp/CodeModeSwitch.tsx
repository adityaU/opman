import React from "react";

export function CodeModeSwitch({ checked, label, busy, onChange }: {
  readonly checked: boolean;
  readonly label: string;
  readonly busy?: boolean;
  readonly onChange: (enabled: boolean) => void;
}) {
  return <button type="button" className="stg-switch cm-settings-switch" role="switch"
    aria-label={label} aria-checked={checked} disabled={busy}
    onClick={() => onChange(!checked)}>
    <span className="stg-switch-track" aria-hidden="true"><span className="stg-switch-knob" /></span>
    <span className="stg-switch-text">{checked ? "On" : "Off"}</span>
  </button>;
}
