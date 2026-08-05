import React, { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ChevronDown, Check, Shield } from "lucide-react";

const PERMISSIONS: Record<string, { value: string; label: string }[]> = {
  "claude-code": [
    { value: "default", label: "Ask when needed" },
    { value: "acceptEdits", label: "Auto-accept edits" },
    { value: "plan", label: "Plan only" },
    { value: "bypassPermissions", label: "Bypass permissions" },
  ],
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

/** Menu width used for viewport clamping; keep in sync with runner-controls.css. */
const MENU_WIDTH = 214;
const EDGE_GAP = 8;

interface Anchor {
  /** Viewport-relative left edge, clamped so the menu never overflows. */
  left: number;
  /** Distance from the viewport's bottom edge (the menu opens upward). */
  bottom: number;
}

function anchorFor(button: HTMLElement): Anchor {
  const rect = button.getBoundingClientRect();
  const maxLeft = Math.max(EDGE_GAP, window.innerWidth - MENU_WIDTH - EDGE_GAP);
  return {
    left: Math.min(Math.max(rect.left, EDGE_GAP), maxLeft),
    bottom: Math.max(EDGE_GAP, window.innerHeight - rect.top + 6),
  };
}

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
  const [anchor, setAnchor] = useState<Anchor | null>(null);
  const btnRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const permissions = PERMISSIONS[currentRunner] || PERMISSIONS.opencode;
  const runnerLabel = currentRunner === "claude-code" ? "Claude Code" : currentRunner === "claude" ? "Claude" : currentRunner;
  const effortLabel = effort ? ` · ${effort}` : "";
  const effortOptions = Array.from(new Set([
    ...supportedEfforts.filter(Boolean),
    ...(effort && !supportedEfforts.includes(effort) ? [effort] : []),
  ]));
  const open = anchor !== null;

  // The chip row is a horizontal scroll container on touch layouts and the
  // composer clips its glass edge, so an absolutely positioned upward menu is
  // never visible there. Portal to <body> with fixed positioning instead.
  useEffect(() => {
    if (!open) return;
    const outside = (event: Event) => {
      const target = event.target as Node;
      if (btnRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      setAnchor(null);
    };
    const reflow = () => {
      if (btnRef.current) setAnchor(anchorFor(btnRef.current));
    };
    document.addEventListener("mousedown", outside);
    document.addEventListener("touchstart", outside, { passive: true });
    window.addEventListener("resize", reflow);
    window.addEventListener("scroll", reflow, true);
    return () => {
      document.removeEventListener("mousedown", outside);
      document.removeEventListener("touchstart", outside);
      window.removeEventListener("resize", reflow);
      window.removeEventListener("scroll", reflow, true);
    };
  }, [open]);

  const toggle = useCallback(() => {
    setAnchor((prev) => (prev || !btnRef.current ? null : anchorFor(btnRef.current)));
  }, []);

  return (
    <div className="prompt-runner-selector">
      <button ref={btnRef} className="prompt-chip prompt-runner-chip" type="button"
        title="Choose runner, effort, and permissions"
        disabled={disabled} aria-haspopup="menu" aria-expanded={open} onClick={toggle}>
        <span className="prompt-runner-label">Runner</span>
        <span className="prompt-chip-label">{runnerLabel}{effortLabel}</span>
        <ChevronDown size={9} />
      </button>
      {anchor && createPortal(
        <div className="prompt-runner-menu" role="menu" ref={menuRef}
          style={{ position: "fixed", left: anchor.left, bottom: anchor.bottom }}>
          <div className="prompt-runner-menu-title">Runner</div>
          {availableRunners.map((runner) => (
            <button key={runner} type="button" role="menuitemradio" aria-checked={runner === currentRunner}
              className={`prompt-runner-option${runner === currentRunner ? " is-selected" : ""}`}
              onClick={() => onRunnerChange?.(runner)}>
              <span>{runner}</span>{runner === currentRunner && <Check size={12} />}
            </button>
          ))}
          {effortOptions.length > 0 && (
            <div className="prompt-runner-setting prompt-effort-setting">
              <span>Effort</span>
              <div className="prompt-effort-options" role="radiogroup" aria-label="Reasoning effort">
                <button type="button" role="radio" aria-checked={!effort} className={!effort ? "is-selected" : ""} onClick={() => onEffortChange?.(null)}>Default</button>
                {effortOptions.map((value) => (
                  <button key={value} type="button" role="radio" aria-checked={effort === value} className={effort === value ? "is-selected" : ""} onClick={() => onEffortChange?.(value)}>
                    {value}
                  </button>
                ))}
              </div>
            </div>
          )}
          <label className="prompt-runner-setting">
            <span><Shield size={11} /> Permissions</span>
            <select aria-label="Runner permissions" value={permission} onChange={(event) => onPermissionChange?.(event.target.value)}>
              {permissions.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
            </select>
          </label>
        </div>,
        document.body,
      )}
    </div>
  );
}
