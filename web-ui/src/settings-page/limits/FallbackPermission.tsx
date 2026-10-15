import React from "react";
import type { PermissionModeOption } from "../../api/session";
import { useProviders } from "../../hooks/useProviders";
import { runnerLabel } from "../../engine-picker/paletteRows";

interface PermissionSelectProps {
  readonly permission: string | null | undefined;
  readonly modes: readonly PermissionModeOption[] | null;
  readonly loading: boolean;
  readonly label: string;
  readonly onChange: (permission: string | null) => void;
}

/** Only the destination runner's reported modes are selectable. */
export function PermissionSelect({ permission, modes, loading, label, onChange }: PermissionSelectProps) {
  const available = modes ?? [];
  const unavailable = Boolean(permission && !available.some((mode) => mode.value === permission));
  return (
    <select className="stg-input" aria-label={label} value={permission ?? ""}
      disabled={loading} onChange={(event) => onChange(event.target.value || null)}>
      <option value="">{loading ? "Loading permissions…" : "Keep compatible permission"}</option>
      {unavailable && <option value={permission ?? ""} disabled>Saved mode unavailable</option>}
      {available.map((mode) => (
        <option key={mode.value} value={mode.value} title={mode.description}>{mode.label}</option>
      ))}
    </select>
  );
}

interface FallbackPermissionProps {
  readonly runner: string;
  readonly permission: string | null | undefined;
  readonly index: number;
  readonly onChange: (permission: string | null) => void;
}

export function FallbackPermission({ runner, permission, index, onChange }: FallbackPermissionProps) {
  const { permissionModes, loading } = useProviders(runner);
  return (
    <label className="stg-field rlim-permission">
      <span className="stg-label">Permission</span>
      <PermissionSelect permission={permission} modes={permissionModes} loading={loading}
        label={`Permission for fallback ${index + 1}, ${runnerLabel(runner)}`} onChange={onChange} />
    </label>
  );
}
