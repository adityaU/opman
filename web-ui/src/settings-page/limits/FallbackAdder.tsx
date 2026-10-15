import React, { useEffect, useMemo, useState } from "react";
import { Plus } from "lucide-react";
import { useProviders, type ProviderCache } from "../../hooks/useProviders";
import { runnerLabel } from "../../engine-picker/paletteRows";
import type { FallbackPair } from "../../runner-limit/fallbacks";
import { PermissionSelect } from "./FallbackPermission";

/** The runner's default model: whatever it picks when no model is named. */
const DEFAULT_MODEL = "";

interface ModelChoice {
  readonly key: string;
  readonly label: string;
  readonly providerID: string;
  readonly modelID: string;
}

/** Every model the runner lists, connected providers first, keyed `provider/model`. */
function useModelChoices(providers: ProviderCache): { choices: ModelChoice[]; loading: boolean } {
  const choices = useMemo(() => {
    const out: (ModelChoice & { connected: boolean })[] = [];
    const multi = providers.all.length > 1;
    for (const provider of providers.all) {
      for (const [modelID, info] of Object.entries(provider.models ?? {})) {
        const name = info.name || modelID;
        out.push({
          key: `${provider.id}/${modelID}`,
          label: multi ? `${name} · ${provider.name || provider.id}` : name,
          providerID: provider.id,
          modelID,
          connected: providers.connected.has(provider.id),
        });
      }
    }
    out.sort((a, b) => Number(b.connected) - Number(a.connected) || a.label.localeCompare(b.label));
    return out;
  }, [providers.all, providers.connected]);
  return { choices, loading: providers.loading };
}

export interface FallbackAdderProps {
  readonly runners: readonly string[];
  readonly onAdd: (pair: FallbackPair) => void;
}

/** Pick a runner, then one of its models (or its default), and add the pair to the list. */
export function FallbackAdder({ runners, onAdd }: FallbackAdderProps) {
  const [runner, setRunner] = useState(runners[0] ?? "");
  const [modelKey, setModelKey] = useState(DEFAULT_MODEL);
  const [permission, setPermission] = useState<string | null>(null);
  const providers = useProviders(runner);
  const { choices, loading } = useModelChoices(providers);

  // The runner list arrives after mount on a cold page; adopt its first entry then.
  useEffect(() => {
    if (!runner && runners[0]) setRunner(runners[0]);
  }, [runner, runners]);

  const add = () => {
    if (!runner) return;
    const choice = choices.find((entry) => entry.key === modelKey);
    const selectedPermission = providers.permissionModes?.some((mode) => mode.value === permission)
      ? permission : null;
    onAdd({ runner, model: choice ? { providerID: choice.providerID, modelID: choice.modelID } : null,
      permission: selectedPermission });
    setModelKey(DEFAULT_MODEL);
    setPermission(null);
  };

  return (
    <div className="stg-form-grid rlim-adder">
      <label className="stg-field">
        <span className="stg-label">Runner</span>
        <select
          className="stg-input"
          value={runner}
          onChange={(event) => {
            setRunner(event.target.value);
            setModelKey(DEFAULT_MODEL);
            setPermission(null);
          }}
        >
          {runners.map((id) => (
            <option key={id} value={id}>{runnerLabel(id)}</option>
          ))}
        </select>
      </label>
      <label className="stg-field">
        <span className="stg-label">Model</span>
        <select
          className="stg-input"
          value={modelKey}
          disabled={!runner}
          onChange={(event) => setModelKey(event.target.value)}
        >
          <option value={DEFAULT_MODEL}>{loading ? "Loading models…" : "Runner's default"}</option>
          {choices.map((choice) => (
            <option key={choice.key} value={choice.key}>{choice.label}</option>
          ))}
        </select>
      </label>
      <label className="stg-field">
        <span className="stg-label">Permission</span>
        <PermissionSelect permission={permission} modes={providers.permissionModes}
          loading={loading} label="Permission for new fallback" onChange={setPermission} />
      </label>
      <div className="stg-field rlim-adder-action">
        <button type="button" className="stg-btn is-primary" onClick={add} disabled={!runner}>
          <Plus size={13} /> Add
        </button>
      </div>
    </div>
  );
}
