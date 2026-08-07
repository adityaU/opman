import React, { useState } from "react";
import { Plus, Undo2, X } from "lucide-react";

/**
 * Name/value pairs whose values opman never sends back — `env` for a child process,
 * `headers` for a remote one.
 *
 * So the editor works the way a password field does: an existing key is shown as set, not
 * shown as text, and changing it means typing a new value. Nothing here can echo a
 * credential it was never given.
 */

export interface SecretEdits {
  /** Keys to add or overwrite. */
  readonly set: Record<string, string>;
  /** Keys to delete. */
  readonly remove: readonly string[];
}

export const NO_EDITS: SecretEdits = { set: {}, remove: [] };

export interface SecretEditorProps {
  readonly label: string;
  /** Keys already in the config. Values are deliberately not available. */
  readonly existing: readonly string[];
  readonly edits: SecretEdits;
  readonly onChange: (edits: SecretEdits) => void;
  readonly namePlaceholder: string;
}

export function SecretEditor(props: SecretEditorProps) {
  const { label, existing, edits, onChange, namePlaceholder } = props;
  const [name, setName] = useState("");
  const [value, setValue] = useState("");

  const removed = new Set(edits.remove);
  const added = Object.keys(edits.set).filter((key) => !existing.includes(key));

  const stage = () => {
    const key = name.trim();
    if (!key) return;
    onChange({
      set: { ...edits.set, [key]: value },
      // Re-adding a key the user just removed should mean "keep it, with this value".
      remove: edits.remove.filter((entry) => entry !== key),
    });
    setName("");
    setValue("");
  };

  const drop = (key: string) => {
    const { [key]: _dropped, ...rest } = edits.set;
    onChange({
      set: rest,
      remove: existing.includes(key) ? [...edits.remove, key] : edits.remove,
    });
  };

  const restore = (key: string) =>
    onChange({ set: edits.set, remove: edits.remove.filter((entry) => entry !== key) });

  return (
    <fieldset className="stg-field">
      <legend className="stg-label">{label}</legend>

      {existing.length === 0 && added.length === 0 && (
        <p className="stg-hint">None set.</p>
      )}

      <ul className="stg-secrets">
        {existing.map((key) => {
          const replaced = key in edits.set;
          return (
            <li key={key} className={removed.has(key) ? "stg-secret is-removed" : "stg-secret"}>
              <code>{key}</code>
              <span className="stg-secret-state">
                {removed.has(key) ? "will be removed" : replaced ? "new value staged" : "set"}
              </span>
              {removed.has(key) ? (
                <button
                  type="button"
                  className="stg-icon-btn"
                  onClick={() => restore(key)}
                  aria-label={`Keep ${key}`}
                >
                  <Undo2 size={13} />
                </button>
              ) : (
                <button
                  type="button"
                  className="stg-icon-btn is-danger"
                  onClick={() => drop(key)}
                  aria-label={`Remove ${key}`}
                >
                  <X size={13} />
                </button>
              )}
            </li>
          );
        })}
        {added.map((key) => (
          <li key={key} className="stg-secret">
            <code>{key}</code>
            <span className="stg-secret-state">will be added</span>
            <button
              type="button"
              className="stg-icon-btn is-danger"
              onClick={() => drop(key)}
              aria-label={`Discard ${key}`}
            >
              <X size={13} />
            </button>
          </li>
        ))}
      </ul>

      <div className="stg-secret-add">
        <input
          className="stg-input"
          value={name}
          placeholder={namePlaceholder}
          spellCheck={false}
          aria-label={`${label} name`}
          onChange={(event) => setName(event.target.value)}
        />
        <input
          className="stg-input"
          type="password"
          value={value}
          placeholder="value"
          autoComplete="off"
          aria-label={`${label} value`}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== "Enter") return;
            event.preventDefault();
            stage();
          }}
        />
        <button type="button" className="stg-btn" onClick={stage} disabled={!name.trim()}>
          <Plus size={13} aria-hidden="true" />
          Set
        </button>
      </div>
      <p className="stg-hint">
        A value may be <code>{"${env:NAME}"}</code> to read it from opman's own environment
        instead of storing it.
      </p>
    </fieldset>
  );
}
