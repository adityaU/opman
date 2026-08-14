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

interface Props {
  /**
   * Modes the engine reported. Null (not asked yet) and empty (asked, has no permission
   * model — native opencode takes no `permission` field at all) both hide the control:
   * every runner that has modes publishes them, so there is nothing to fill in from here.
   */
  permissionModes: PermissionModeOption[] | null;
  supportedEfforts: string[];
  effort: string | null;
  permission: string;
  onEffortChange: (effort: string | null) => void;
  onPermissionChange: (permission: string) => void;
}

export function EngineSettingsRow({
  permissionModes, supportedEfforts, effort, permission, onEffortChange, onPermissionChange,
}: Props) {
  const permissions = permissionModes ?? [];
  const efforts = Array.from(new Set([
    ...supportedEfforts.filter(Boolean),
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
