import React, { useCallback, useEffect, useRef, useState } from "react";
import { FileUp, Plus, Trash2 } from "lucide-react";
import { fetchMcpServers } from "../../api/mcp";
import type { Skill } from "../../api/skills";
import { SkillForm } from "./SkillForm";
import { useSkills } from "./useSkills";

/**
 * Skills: reusable instructions any runner can load.
 *
 * This is the first UI they have had beyond a zip upload — which is now one button here
 * rather than a modal of its own, because uploading is a way of creating a skill, not a
 * separate feature.
 */

type Editor =
  | { readonly kind: "none" }
  | { readonly kind: "new" }
  | { readonly kind: "loading"; readonly name: string }
  | { readonly kind: "skill"; readonly skill: Skill };

export interface SkillsSectionProps {
  readonly onError: (message: string) => void;
}

export function SkillsSection({ onError }: SkillsSectionProps) {
  const state = useSkills(onError);
  const [editor, setEditor] = useState<Editor>({ kind: "none" });
  const [confirming, setConfirming] = useState<string>();
  const [servers, setServers] = useState<readonly string[]>([]);
  const filePicker = useRef<HTMLInputElement>(null);

  // Only the names, so `requires` can be picked from real servers rather than typed.
  useEffect(() => {
    let alive = true;
    fetchMcpServers()
      .then((list) => alive && setServers(list.map((server) => server.name)))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

  const close = useCallback(() => setEditor({ kind: "none" }), []);

  const edit = useCallback(
    async (name: string) => {
      if (editor.kind === "skill" && editor.skill.name === name) {
        close();
        return;
      }
      setEditor({ kind: "loading", name });
      const skill = await state.load(name);
      if (!skill) {
        setEditor({ kind: "none" });
        onError(`${name} could not be read`);
        return;
      }
      setEditor({ kind: "skill", skill });
    },
    [editor, close, state, onError],
  );

  const remove = useCallback(
    async (name: string) => {
      setConfirming(undefined);
      await state.remove(name);
      close();
    },
    [state, close],
  );

  const pick = useCallback(
    async (event: React.ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      // Reset first: picking the same file twice in a row fires no change event otherwise.
      event.target.value = "";
      if (!file) return;
      if (!file.name.toLowerCase().endsWith(".zip")) {
        onError("Skills are installed from a .zip archive");
        return;
      }
      await state.upload(file);
    },
    [state, onError],
  );

  return (
    <div className="stg-stack">
      <section className="stg-card">
        <div className="stg-card-head">
          <div>
            <h3 className="stg-card-title">Installed skills</h3>
            <p className="stg-card-note">
              Each skill is a folder holding a <code>SKILL.md</code>. Every runner reaches
              them through opman's own MCP server, so one copy serves all four.
            </p>
          </div>
          <div className="stg-card-actions">
            <button
              type="button"
              className="stg-btn"
              onClick={() => filePicker.current?.click()}
              disabled={state.saving}
            >
              <FileUp size={13} aria-hidden="true" />
              Import zip
            </button>
            <button
              type="button"
              className="stg-btn is-primary"
              onClick={() => setEditor(editor.kind === "new" ? { kind: "none" } : { kind: "new" })}
              aria-expanded={editor.kind === "new"}
            >
              <Plus size={13} aria-hidden="true" />
              New skill
            </button>
          </div>
        </div>

        <input
          ref={filePicker}
          type="file"
          accept=".zip"
          hidden
          onChange={pick}
          aria-label="Skills archive"
        />

        {editor.kind === "new" && (
          <SkillForm
            servers={servers}
            saving={state.saving}
            onSubmit={state.save}
            onCancel={close}
          />
        )}

        {state.error && (
          <p className="stg-error" role="alert">
            {state.error}
          </p>
        )}

        {state.loading ? (
          <p className="stg-hint">Loading…</p>
        ) : state.skills.length === 0 ? (
          <div className="stg-empty">
            <p>No skills yet.</p>
            <p className="stg-hint">
              A skill is a description of when to use it and a body of instructions. Write
              one, or import a folder of them as a zip.
            </p>
          </div>
        ) : (
          <ul className="stg-rows">
            {state.skills.map((skill) => {
              const open = editor.kind === "skill" && editor.skill.name === skill.name;
              return (
                <React.Fragment key={skill.name}>
                  <li className="stg-row">
                    <button
                      type="button"
                      className="stg-row-main stg-row-open"
                      onClick={() => edit(skill.name)}
                      aria-expanded={open}
                    >
                      <span className="stg-row-head">
                        <span className="stg-row-name">{skill.title || skill.name}</span>
                        {skill.title && skill.title !== skill.name && (
                          <span className="stg-tag">{skill.name}</span>
                        )}
                        {skill.requires.map((server) => (
                          <span key={server} className="stg-tag is-accent">
                            needs {server}
                          </span>
                        ))}
                        {editor.kind === "loading" && editor.name === skill.name && (
                          <span className="stg-tag">opening…</span>
                        )}
                      </span>
                      <span className="stg-row-origin">{skill.description}</span>
                    </button>
                    <div className="stg-row-actions">
                      <button
                        type="button"
                        className="stg-icon-btn is-danger"
                        onClick={() => setConfirming(skill.name)}
                        aria-label={`Delete ${skill.name}`}
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </li>
                  {confirming === skill.name && (
                    <li className="stg-confirm">
                      <span>
                        Delete <strong>{skill.name}</strong>? Its folder and everything in it
                        is removed.
                      </span>
                      <span className="stg-confirm-actions">
                        <button
                          type="button"
                          className="stg-btn is-danger"
                          onClick={() => remove(skill.name)}
                        >
                          Delete
                        </button>
                        <button
                          type="button"
                          className="stg-btn"
                          onClick={() => setConfirming(undefined)}
                        >
                          Keep
                        </button>
                      </span>
                    </li>
                  )}
                  {open && (
                    <li className="stg-row-form">
                      <SkillForm
                        skill={editor.skill}
                        servers={servers}
                        saving={state.saving}
                        onSubmit={state.save}
                        onCancel={close}
                      />
                    </li>
                  )}
                </React.Fragment>
              );
            })}
          </ul>
        )}
      </section>
    </div>
  );
}
