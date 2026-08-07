import React, { useCallback, useMemo, useState } from "react";
import type { Skill, SkillDraft } from "../../api/skills";

/**
 * Write one skill.
 *
 * The name is the directory name and therefore the identity — renaming one writes the new
 * directory and drops the old, which is why it is spelled out here rather than treated as
 * a cosmetic edit.
 */

const NAME_PATTERN = /^[a-z0-9][a-z0-9._-]*$/;

export interface SkillFormProps {
  /** Absent when writing a new skill. */
  readonly skill?: Skill;
  /** MCP servers a skill can declare a dependency on. */
  readonly servers: readonly string[];
  readonly saving: boolean;
  readonly onSubmit: (draft: SkillDraft, original?: string) => Promise<boolean>;
  readonly onCancel: () => void;
}

export function SkillForm({ skill, servers, saving, onSubmit, onCancel }: SkillFormProps) {
  const [name, setName] = useState(skill?.name ?? "");
  const [title, setTitle] = useState(skill?.title ?? "");
  const [description, setDescription] = useState(skill?.description ?? "");
  const [content, setContent] = useState(skill?.content ?? "");
  const [requires, setRequires] = useState<readonly string[]>(skill?.requires ?? []);
  const [problem, setProblem] = useState<string>();

  // A skill may already require a server that has since been removed from mcp.json. It
  // stays offered so saving cannot silently drop the dependency.
  const options = useMemo(() => {
    const all = new Set([...servers, ...requires]);
    return [...all].sort();
  }, [servers, requires]);

  const invalid = useMemo(() => {
    const trimmed = name.trim().toLowerCase();
    if (!trimmed) return "A name is required.";
    if (!NAME_PATTERN.test(trimmed)) {
      return "Use lowercase letters, digits, dots, dashes or underscores.";
    }
    if (!description.trim()) return "A description is required — it is what an agent reads to decide whether to load the skill.";
    return undefined;
  }, [name, description]);

  const toggle = (server: string) =>
    setRequires((current) =>
      current.includes(server)
        ? current.filter((entry) => entry !== server)
        : [...current, server],
    );

  const submit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (invalid) {
        setProblem(invalid);
        return;
      }
      setProblem(undefined);
      const draft: SkillDraft = {
        name: name.trim().toLowerCase(),
        title: title.trim(),
        description: description.trim(),
        content,
        requires: [...requires],
      };
      if (await onSubmit(draft, skill?.name)) onCancel();
    },
    [invalid, name, title, description, content, requires, onSubmit, skill, onCancel],
  );

  const renaming = skill !== undefined && skill.name !== name.trim().toLowerCase();

  return (
    <form className="stg-form" onSubmit={submit}>
      <div className="stg-form-grid">
        <div className="stg-field">
          <label className="stg-label" htmlFor="stg-skill-name">
            Name
          </label>
          <input
            id="stg-skill-name"
            className="stg-input"
            value={name}
            placeholder="release-notes"
            spellCheck={false}
            onChange={(event) => setName(event.target.value)}
          />
          <p className="stg-hint">
            {renaming
              ? `Saving writes ${name.trim().toLowerCase()} and removes ${skill?.name}.`
              : "The directory name under ~/.config/opman/skills."}
          </p>
        </div>

        <div className="stg-field">
          <label className="stg-label" htmlFor="stg-skill-title">
            Title
          </label>
          <input
            id="stg-skill-title"
            className="stg-input"
            value={title}
            placeholder="Optional display name"
            onChange={(event) => setTitle(event.target.value)}
          />
          <p className="stg-hint">Shown to the agent. Defaults to the name.</p>
        </div>
      </div>

      <div className="stg-field">
        <label className="stg-label" htmlFor="stg-skill-description">
          Description
        </label>
        <textarea
          id="stg-skill-description"
          className="stg-input stg-textarea"
          rows={2}
          value={description}
          placeholder="When to use this skill, in one or two sentences."
          onChange={(event) => setDescription(event.target.value)}
        />
        <p className="stg-hint">
          This is the whole basis for an agent deciding to load the skill, so say when it
          applies rather than what it contains.
        </p>
      </div>

      {options.length > 0 && (
        <fieldset className="stg-field">
          <legend className="stg-label">Requires</legend>
          <div className="stg-checks">
            {options.map((server) => (
              <label key={server} className="stg-check">
                <input
                  type="checkbox"
                  checked={requires.includes(server)}
                  onChange={() => toggle(server)}
                />
                {server}
              </label>
            ))}
          </div>
          <p className="stg-hint">
            MCP servers this skill needs, so opman can name the missing login instead of the
            agent failing halfway through.
          </p>
        </fieldset>
      )}

      <div className="stg-field">
        <label className="stg-label" htmlFor="stg-skill-content">
          Instructions
        </label>
        <textarea
          id="stg-skill-content"
          className="stg-input stg-textarea is-tall"
          rows={16}
          value={content}
          placeholder="Markdown. This is the body an agent receives when it loads the skill."
          spellCheck={false}
          onChange={(event) => setContent(event.target.value)}
        />
      </div>

      {problem && (
        <p className="stg-error" role="alert">
          {problem}
        </p>
      )}

      <div className="stg-form-actions">
        <button type="submit" className="stg-btn is-primary" disabled={saving}>
          {saving ? "Saving…" : skill ? "Save skill" : "Create skill"}
        </button>
        <button type="button" className="stg-btn" onClick={onCancel} disabled={saving}>
          Cancel
        </button>
      </div>
    </form>
  );
}
