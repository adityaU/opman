import React, { useState } from "react";
import { ArrowDown, ArrowUp, X } from "lucide-react";
import type { FallbackPair, ModelSet } from "../../api/runnerLimits";
import { samePair } from "../../runner-limit/fallbacks";
import { runnerLabel } from "../../engine-picker/paletteRows";
import { FallbackAdder } from "./FallbackAdder";
import { FallbackPermission } from "./FallbackPermission";

interface Props {
  readonly initial: ModelSet;
  readonly sets: readonly ModelSet[];
  readonly runners: readonly string[];
  readonly onSave: (set: ModelSet) => Promise<void>;
  readonly onCancel: () => void;
}

export function ModelSetEditor({ initial, sets, runners, onSave, onCancel }: Props) {
  const [name, setName] = useState(initial.name);
  const [members, setMembers] = useState(initial.members);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const add = (member: FallbackPair) => {
    if (!member.model) return;
    const conflict = sets.find(set => set.id !== initial.id && set.members.some(entry => samePair(entry, member)));
    if (conflict) {
      setError(`This runner and model already belong to “${conflict.name}”. Remove them there first.`);
      return;
    }
    setError("");
    const existing = members.findIndex(entry => entry.runner === member.runner);
    setMembers(existing < 0 ? [...members, member]
      : members.map((entry, index) => index === existing ? member : entry));
  };
  const move = (index: number, offset: number) => {
    const target = index + offset;
    if (target < 0 || target >= members.length) return;
    const next = [...members];
    [next[index], next[target]] = [next[target], next[index]];
    setMembers(next);
  };
  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim() || members.length < 2 || saving) return;
    if (sets.some(set => set.id !== initial.id && set.name.toLowerCase() === name.trim().toLowerCase())) {
      setError("Choose a different name. A set already uses this name.");
      return;
    }
    setSaving(true);
    setError("");
    try {
      await onSave({ ...initial, name: name.trim(), members });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not save this model set.");
    } finally {
      setSaving(false);
    }
  };
  return <form className="rlim-set-editor" onSubmit={event => void save(event)}>
    <fieldset disabled={saving} className="rlim-set-fields">
      <label className="stg-field">
        <span className="stg-label">Set name</span>
        <input className="stg-input" value={name} maxLength={80} required
          placeholder="For example, Deep work" onChange={event => setName(event.target.value)} />
      </label>
      <p className="stg-card-note">Add at least two runners and choose a model for each.
        Adding another model for a runner replaces its entry. Order decides which runner is tried first.</p>
      <ol className="rlim-list" aria-label="Models in this set">
        {members.map((member, index) => <li className="rlim-item" key={member.runner}>
          <span className="rlim-rank">{index + 1}</span>
          <span className="rlim-runner">{runnerLabel(member.runner)}</span>
          <span className="rlim-model" title={member.model?.modelID}>{member.model?.modelID}</span>
          <FallbackPermission runner={member.runner} permission={member.permission} index={index}
            label={`Permission for ${runnerLabel(member.runner)} in this set`}
            onChange={permission => setMembers(members.map((entry, at) =>
              at === index ? { ...entry, permission } : entry))} />
          <span className="rlim-item-actions">
            <button className="stg-icon-btn" type="button" aria-label={`Move ${runnerLabel(member.runner)} up`}
              disabled={index === 0} onClick={() => move(index, -1)}><ArrowUp size={13} /></button>
            <button className="stg-icon-btn" type="button" aria-label={`Move ${runnerLabel(member.runner)} down`}
              disabled={index === members.length - 1} onClick={() => move(index, 1)}><ArrowDown size={13} /></button>
            <button className="stg-icon-btn" type="button" aria-label={`Remove ${runnerLabel(member.runner)} from set`}
              onClick={() => setMembers(members.filter((_, at) => at !== index))}><X size={13} /></button>
          </span>
        </li>)}
      </ol>
      <FallbackAdder runners={runners} requireModel onAdd={add} />
      {error && <p className="stg-error" role="alert">{error}</p>}
      <div className="rlim-set-actions">
        <button type="submit" className="stg-btn is-primary" disabled={!name.trim() || members.length < 2}>
          {saving ? "Saving…" : "Save set"}
        </button>
        <button type="button" className="stg-btn" onClick={onCancel}>Cancel</button>
      </div>
    </fieldset>
  </form>;
}
