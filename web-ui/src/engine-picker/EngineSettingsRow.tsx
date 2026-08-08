/**
 * Effort and permission for the current runner.
 *
 * These are settings of the chosen engine rather than choices in their own
 * right, so they sit below the list as a footer instead of competing with it —
 * and they move with the runner, because "auto-accept edits" means nothing to
 * a runner that has no permission model.
 */
import { Shield } from "lucide-react";
import type { PermissionModeOption } from "../api/session";

/**
 * Fallback permission modes, by runner name. Engines that report their own modes (every ACP
 * agent does) override this — a config-declared agent can never appear in a table like
 * this, so discovery is the rule and these are the backstop.
 */
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
  opencode: [{ value: "default", label: "Default" }],
};

interface Props {
  runner: string;
  /**
   * Modes the engine reported, or null to fall back to the table above. An empty array is
   * not the same as null: it means the engine was asked and has no permission model to
   * offer, so the control is hidden rather than filled in with someone else's modes.
   */
  permissionModes: PermissionModeOption[] | null;
  supportedEfforts: string[];
  effort: string | null;
  permission: string;
  onEffortChange: (effort: string | null) => void;
  onPermissionChange: (permission: string) => void;
}

export function EngineSettingsRow({
  runner, permissionModes, supportedEfforts, effort, permission, onEffortChange, onPermissionChange,
}: Props) {
  const permissions = permissionModes ?? PERMISSIONS[runner] ?? PERMISSIONS.opencode;
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
      {permissions.length > 0 && (
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
      )}
    </div>
  );
}
