import React, { useId, useState } from "react";
import { ChevronRight } from "lucide-react";
import type { NativeToolsInfo, NativeToolsSetting } from "../../api/acp";
import { afterMasterClick, afterToolClick, masterState } from "./nativeToolsModel";

/**
 * An agent's own built-in tools, with a master switch and one switch per tool.
 *
 * The point is to let an agent use opman's MCP tools instead of its own shell, editor and
 * web access. A harness opman has no way to control gets one line saying so, never a row of
 * switches that would do nothing.
 */

export interface NativeToolsPanelProps {
  readonly agentName: string;
  readonly info: NativeToolsInfo | null;
  readonly disabled: boolean;
  readonly onChange: (setting: NativeToolsSetting) => void;
}

function Switch({
  checked,
  mixed = false,
  master = false,
  label,
  text,
  disabled,
  onClick,
}: {
  readonly checked: boolean;
  readonly mixed?: boolean;
  readonly master?: boolean;
  readonly label: string;
  readonly text: string;
  readonly disabled: boolean;
  readonly onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="stg-switch"
      role={master ? "checkbox" : "switch"}
      aria-checked={mixed ? "mixed" : checked}
      data-mixed={mixed ? "true" : undefined}
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
    >
      <span className="stg-switch-track" aria-hidden="true">
        <span className="stg-switch-knob" />
      </span>
      <span className="stg-switch-text">{text}</span>
    </button>
  );
}

export function NativeToolsPanel({ agentName, info, disabled, onChange }: NativeToolsPanelProps) {
  const [open, setOpen] = useState(false);
  const listId = useId();

  if (!info) {
    return (
      <p className="stg-tools-none">
        Native tools can't be controlled for this agent. opman has no tested way to switch them
        off.
      </p>
    );
  }

  const state = masterState(info.tools);
  const on = info.tools.filter((tool) => tool.enabled).length;

  return (
    <section className="stg-tools" aria-label={`Native tools of ${agentName}`}>
      <div className="stg-tools-head">
        <button
          type="button"
          className="stg-tools-toggle"
          aria-expanded={open}
          aria-controls={listId}
          onClick={() => setOpen((value) => !value)}
        >
          <ChevronRight size={13} aria-hidden="true" />
          <span className="stg-tools-title">Native tools</span>
          <span className="stg-tools-count">
            {on} of {info.tools.length} on
          </span>
        </button>
        <Switch
          master
          checked={state === "all"}
          mixed={state === "some"}
          label={`All native tools of ${agentName}`}
          text={state === "all" ? "All on" : state === "none" ? "All off" : "Mixed"}
          disabled={disabled}
          onClick={() => onChange(afterMasterClick(info.tools))}
        />
      </div>
      <p className="stg-tools-note">
        Controls the supported tools listed here. Other tools may still be available from the
        agent. Switch these off to use opman's MCP tools instead. Applies to new
        sessions. {info.note}
      </p>
      {open && (
        <ul id={listId} className="stg-tools-list">
          {info.tools.map((tool) => (
            <li key={tool.id} className="stg-tools-item">
              <span className="stg-tools-text">
                <span className="stg-tools-label">{tool.label}</span>
                <span className="stg-tools-desc">{tool.description}</span>
              </span>
              <Switch
                checked={tool.enabled}
                label={`${tool.label} for ${agentName}`}
                text={tool.enabled ? "On" : "Off"}
                disabled={disabled}
                onClick={() => onChange(afterToolClick(info.tools, tool.id))}
              />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
