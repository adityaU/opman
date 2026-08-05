/**
 * Effort and permission for the current runner.
 *
 * These are settings of the chosen engine rather than choices in their own
 * right, so they sit below the list as a footer instead of competing with it —
 * and they move with the runner, because "auto-accept edits" means nothing to
 * a runner that has no permission model.
 */
import { Shield } from "lucide-react";

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

interface Props {
  runner: string;
  supportedEfforts: string[];
  effort: string | null;
  permission: string;
  onEffortChange: (effort: string | null) => void;
  onPermissionChange: (permission: string) => void;
}

export function EngineSettingsRow({
  runner, supportedEfforts, effort, permission, onEffortChange, onPermissionChange,
}: Props) {
  const permissions = PERMISSIONS[runner] || PERMISSIONS.opencode;
  const efforts = Array.from(new Set([
    ...supportedEfforts.filter(Boolean),
    ...(effort && !supportedEfforts.includes(effort) ? [effort] : []),
  ]));

  return (
    <div className="engine-settings">
      {efforts.length > 0 && (
        <div className="engine-setting">
          <span className="engine-setting-label">Effort</span>
          <div className="engine-effort" role="radiogroup" aria-label="Reasoning effort">
            <button
              type="button"
              role="radio"
              aria-checked={!effort}
              className={!effort ? "is-selected" : ""}
              onClick={() => onEffortChange(null)}
            >
              Default
            </button>
            {efforts.map((value) => (
              <button
                key={value}
                type="button"
                role="radio"
                aria-checked={effort === value}
                className={effort === value ? "is-selected" : ""}
                onClick={() => onEffortChange(value)}
              >
                {value}
              </button>
            ))}
          </div>
        </div>
      )}
      <label className="engine-setting">
        <span className="engine-setting-label"><Shield size={11} /> Permissions</span>
        <select
          className="engine-permission"
          aria-label="Runner permissions"
          value={permission}
          onChange={(event) => onPermissionChange(event.target.value)}
        >
          {permissions.map((item) => (
            <option key={item.value} value={item.value}>{item.label}</option>
          ))}
        </select>
      </label>
    </div>
  );
}
