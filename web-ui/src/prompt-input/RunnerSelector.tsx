import React, { useEffect, useRef, useState } from "react";
import { ChevronDown, Check, Shield } from "lucide-react";

const PERMISSIONS: Record<string, { value: string; label: string }[]> = {
  claude: [
    { value: "default", label: "Ask when needed" },
    { value: "acceptEdits", label: "Auto-accept edits" },
    { value: "plan", label: "Plan only" },
    { value: "bypassPermissions", label: "Bypass permissions" },
  ],
  codex: [
    { value: "on-request", label: "Ask when needed" },
    { value: "never", label: "Never ask" },
    { value: "on-failure", label: "Ask after failure" },
    { value: "untrusted", label: "Untrusted only" },
  ],
  opencode: [{ value: "default", label: "Default" }],
};

interface Props {
  currentRunner: string;
  availableRunners: string[];
  supportedEfforts: string[];
  effort: string | null;
  permission: string;
  disabled: boolean;
  onRunnerChange?: (runner: string) => void;
  onEffortChange?: (effort: string | null) => void;
  onPermissionChange?: (permission: string) => void;
}

export function RunnerSelector({
  currentRunner, availableRunners, supportedEfforts, effort, permission, disabled,
  onRunnerChange, onEffortChange, onPermissionChange,
}: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const permissions = PERMISSIONS[currentRunner] || PERMISSIONS.opencode;
  const effortLabel = effort ? ` · ${effort}` : "";
  const permissionLabel = permissions.find((item) => item.value === permission)?.label || permission;

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  useEffect(() => {
    if (!supportedEfforts.includes(effort || "")) onEffortChange?.(null);
  }, [supportedEfforts, effort, onEffortChange]);

  return (
    <div className="prompt-runner-selector" ref={ref}>
      <button className="prompt-chip prompt-runner-chip" type="button" title="Choose runner, effort, and permissions"
        disabled={disabled} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
        <span className="prompt-runner-label">Runner</span>
        <span>{currentRunner}{effortLabel}</span>
        <ChevronDown size={9} />
      </button>
      {open && (
        <div className="prompt-runner-menu" role="menu">
          <div className="prompt-runner-menu-title">Runner</div>
          {availableRunners.map((runner) => (
            <button key={runner} type="button" role="menuitemradio" aria-checked={runner === currentRunner}
              className={`prompt-runner-option${runner === currentRunner ? " is-selected" : ""}`}
              onClick={() => onRunnerChange?.(runner)}>
              <span>{runner}</span>{runner === currentRunner && <Check size={12} />}
            </button>
          ))}
          {supportedEfforts.length > 0 && (
            <label className="prompt-runner-setting">
              <span>Effort</span>
              <select aria-label="Reasoning effort" value={effort || ""} onChange={(event) => onEffortChange?.(event.target.value || null)}>
                <option value="">Default</option>
                {supportedEfforts.map((value) => <option key={value} value={value}>{value}</option>)}
              </select>
            </label>
          )}
          <label className="prompt-runner-setting">
            <span><Shield size={11} /> Permissions</span>
            <select aria-label="Runner permissions" value={permission} onChange={(event) => onPermissionChange?.(event.target.value)}>
              {permissions.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
            </select>
          </label>
        </div>
      )}
    </div>
  );
}
