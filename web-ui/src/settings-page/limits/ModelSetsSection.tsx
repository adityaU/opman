import React, { useState } from "react";
import { Pencil, Plus, Trash2 } from "lucide-react";
import type { ModelSet } from "../../api/runnerLimits";
import { useFallbacks } from "../../runner-limit/fallbacks";
import { getSettings, saveSettings } from "../../runner-limit/settingsStore";
import { runnerLabel } from "../../engine-picker/paletteRows";
import { ModelSetEditor } from "./ModelSetEditor";
import "../../styles/model-sets.css";

export function ModelSetsSection({ runners }: { runners: readonly string[] }) {
  const { modelSets = [] } = useFallbacks();
  const [draft, setDraft] = useState<ModelSet | null>(null);
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState<string | null>(null);
  const save = async (set: ModelSet) => {
    const sets = getSettings().modelSets ?? [];
    const exists = sets.some(entry => entry.id === set.id);
    await saveSettings({ modelSets: exists ? sets.map(entry => entry.id === set.id ? set : entry) : [...sets, set] });
    setDraft(null);
  };
  const remove = async (id: string) => {
    setDeleting(id);
    setError("");
    try {
      await saveSettings({ modelSets: (getSettings().modelSets ?? []).filter(set => set.id !== id) });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not remove this model set.");
    } finally {
      setDeleting(null);
    }
  };
  return <section className="stg-card" aria-label="Model sets">
    <div className="stg-card-head">
      <h3 className="stg-card-title">Model sets</h3>
      <span className="stg-count">{modelSets.length}</span>
    </div>
    <p className="stg-card-note">
      Keep related models together across runners. When the current model belongs to a set,
      opman suggests or switches to another model in that same set. Runners that are low
      or share the same account are skipped.
    </p>
    {modelSets.length === 0 && !draft &&
      <p className="rlim-empty">No sets yet. For example, put your deep-work models in one set and faster models in another.</p>}
    <ul className="rlim-set-list">
      {modelSets.filter(set => set.id !== draft?.id).map(set => <li key={set.id} className="rlim-set">
        <div className="rlim-set-description">
          <strong>{set.name}</strong>
          <span>{set.members.map(member =>
            `${runnerLabel(member.runner)} · ${member.model?.modelID ?? "unavailable model"}`).join(" / ")}</span>
        </div>
        <span className="rlim-item-actions">
          <button className="stg-icon-btn" type="button" aria-label={`Edit ${set.name}`}
            disabled={Boolean(draft) || deleting !== null} onClick={() => setDraft(set)}><Pencil size={13} /></button>
          <button className="stg-icon-btn" type="button" aria-label={`Delete ${set.name}`}
            disabled={Boolean(draft) || deleting !== null} onClick={() => void remove(set.id)}><Trash2 size={13} /></button>
        </span>
      </li>)}
    </ul>
    {error && <p className="stg-error" role="alert">{error}</p>}
    {draft
      ? <ModelSetEditor key={draft.id} initial={draft} sets={modelSets} runners={runners}
          onSave={save} onCancel={() => setDraft(null)} />
      : <button type="button" className="stg-btn" disabled={deleting !== null}
          onClick={() => setDraft({ id: crypto.randomUUID(), name: "", members: [] })}>
          <Plus size={13} /> New model set
        </button>}
    <p className="stg-card-note">
      If a model has no set, opman uses the fallback list below. If a matching set has no
      runner with room, it waits rather than choosing a model from another set.
    </p>
  </section>;
}
