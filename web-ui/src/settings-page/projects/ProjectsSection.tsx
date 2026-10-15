import React, { useCallback, useEffect, useState } from "react";
import { FolderGit2, Plus, Trash2 } from "lucide-react";
import { fetchAppState, removeProject, type ProjectInfo } from "../../api";
import { AddProjectModal } from "../../AddProjectModal";

function sessionCount(project: ProjectInfo): string {
  const count = project.session_count ?? project.sessions.length;
  return count === 1 ? "1 session" : `${count} sessions`;
}

export interface ProjectsSectionProps {
  readonly onError: (message: string) => void;
}

/**
 * The project folders this server knows. Adding opens the same folder picker the
 * sidebar uses; removing takes a project off the list and leaves its folder and its
 * sessions on disk, so adding it back brings everything with it.
 */
export function ProjectsSection({ onError }: ProjectsSectionProps) {
  const [projects, setProjects] = useState<ProjectInfo[] | null>(null);
  const [adding, setAdding] = useState(false);
  const [confirming, setConfirming] = useState<number | null>(null);

  const reload = useCallback(async () => {
    try {
      setProjects((await fetchAppState()).projects);
    } catch (error) {
      onError(error instanceof Error ? error.message : "Could not read the project list");
    }
  }, [onError]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const remove = async (index: number) => {
    setConfirming(null);
    try {
      await removeProject(index);
    } catch (error) {
      onError(error instanceof Error ? error.message : "Could not remove the project");
    }
    await reload();
  };

  return (
    <div className="stg-stack">
      <section className="stg-card">
        <div className="stg-card-head">
          <div>
            <h3 className="stg-card-title">Projects on this server</h3>
            <p className="stg-card-note">
              Folders opman works in. Removing one only takes it off this list.
            </p>
          </div>
          <div className="stg-card-actions">
            <button type="button" className="stg-btn is-primary" onClick={() => setAdding(true)}>
              <Plus size={13} aria-hidden="true" />
              Add project
            </button>
          </div>
        </div>

        {projects === null ? (
          <p className="stg-hint">Loading…</p>
        ) : projects.length === 0 ? (
          <p className="stg-hint">No projects yet.</p>
        ) : (
          <ul className="stg-rows">
            {projects.map((project) => (
              <React.Fragment key={project.path}>
                <li className="stg-row">
                  <div className="stg-row-main">
                    <span className="stg-row-head">
                      <FolderGit2 size={14} aria-hidden="true" />
                      <span className="stg-row-name">{project.name}</span>
                      {project.git_branch && <span className="stg-tag">{project.git_branch}</span>}
                      <span className="stg-tag">{sessionCount(project)}</span>
                    </span>
                    <span className="stg-row-origin">{project.path}</span>
                  </div>
                  <div className="stg-row-actions">
                    <button
                      type="button"
                      className="stg-icon-btn"
                      onClick={() => setConfirming(project.index)}
                      aria-label={`Remove ${project.name}`}
                      title={`Remove ${project.name}`}
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                </li>
                {confirming === project.index && (
                  <li className="stg-confirm">
                    <span>
                      Remove <strong>{project.name}</strong> from opman? The folder and its
                      sessions stay on disk.
                    </span>
                    <span className="stg-confirm-actions">
                      <button type="button" className="stg-btn is-danger" onClick={() => void remove(project.index)}>
                        Remove
                      </button>
                      <button type="button" className="stg-btn" onClick={() => setConfirming(null)}>
                        Keep
                      </button>
                    </span>
                  </li>
                )}
              </React.Fragment>
            ))}
          </ul>
        )}
      </section>
      {adding && <AddProjectModal onClose={() => setAdding(false)} onProjectAdded={reload} />}
    </div>
  );
}
