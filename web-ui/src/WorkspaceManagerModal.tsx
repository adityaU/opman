import React, { useState, useEffect, useCallback, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { X, Save, Trash2, Play, Layers, Pencil } from "lucide-react";
import type { WorkspaceSnapshot } from "./api";
import { fetchWorkspaces, saveWorkspace, deleteWorkspace } from "./api";

/** Pre-built task templates. */
const TASK_TEMPLATES: WorkspaceSnapshot[] = [
  {
    name: "Debug Mode",
    created_at: "",
    panels: { sidebar: false, terminal: true, editor: true, git: false },
    layout: { sidebar_width: 0, terminal_height: 300, side_panel_width: 0 },
    open_files: [],
    active_file: null,
    terminal_tabs: [{ label: "Test Runner", kind: "shell" }],
    session_id: null,
    git_branch: null,
    is_template: true,
    recipe_description: "Open the editor and terminal for active debugging.",
    recipe_next_action: "Run tests and inspect the active failure.",
    is_recipe: true,
  },
  {
    name: "Review Mode",
    created_at: "",
    panels: { sidebar: false, terminal: false, editor: true, git: true },
    layout: { sidebar_width: 0, terminal_height: 0, side_panel_width: 480 },
    open_files: [],
    active_file: null,
    terminal_tabs: [],
    session_id: null,
    git_branch: null,
    is_template: true,
    recipe_description: "Open review-focused panels for git inspection.",
    recipe_next_action: "Review diffs and summarize the branch state.",
    is_recipe: true,
  },
  {
    name: "Terminal Focus",
    created_at: "",
    panels: { sidebar: false, terminal: true, editor: false, git: false },
    layout: { sidebar_width: 0, terminal_height: 600, side_panel_width: 0 },
    open_files: [],
    active_file: null,
    terminal_tabs: [{ label: "Shell", kind: "shell" }],
    session_id: null,
    git_branch: null,
    is_template: true,
    recipe_description: "Maximize terminal space for shell-driven work.",
    recipe_next_action: "Run the next command-line workflow.",
    is_recipe: true,
  },
];

interface Props {
  onClose: () => void;
  onRestore: (snapshot: WorkspaceSnapshot) => void;
  onSaveCurrent: () => WorkspaceSnapshot;
  activeWorkspaceName: string | null;
}

export function WorkspaceManagerModal({
  onClose,
  onRestore,
  onSaveCurrent,
  activeWorkspaceName,
}: Props) {
  const [workspaces, setWorkspaces] = useState<WorkspaceSnapshot[]>([]);
  const [loading, setLoading] = useState(true);
  const [saveName, setSaveName] = useState("");
  const [recipeDescription, setRecipeDescription] = useState("");
  const [recipeNextAction, setRecipeNextAction] = useState("");
  const [saveAsRecipe, setSaveAsRecipe] = useState(false);
  const [editingRecipeName, setEditingRecipeName] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const loadWorkspaces = useCallback(async () => {
    try {
      const resp = await fetchWorkspaces();
      setWorkspaces(resp.workspaces);
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadWorkspaces();
  }, [loadWorkspaces]);

  const handleSave = useCallback(async () => {
    const name = saveName.trim();
    if (!name) return;
    setSaving(true);
    try {
      const snapshot = onSaveCurrent();
      snapshot.name = name;
      snapshot.created_at = new Date().toISOString();
      snapshot.is_recipe = saveAsRecipe;
      snapshot.recipe_description = saveAsRecipe ? recipeDescription.trim() : null;
      snapshot.recipe_next_action = saveAsRecipe ? recipeNextAction.trim() : null;
      if (editingRecipeName && editingRecipeName !== name) {
        await deleteWorkspace(editingRecipeName);
      }
      await saveWorkspace(snapshot);
      setSaveName("");
      setRecipeDescription("");
      setRecipeNextAction("");
      setSaveAsRecipe(false);
      setEditingRecipeName(null);
      await loadWorkspaces();
    } catch {
      // ignore
    } finally {
      setSaving(false);
    }
  }, [saveName, onSaveCurrent, loadWorkspaces]);

  const handleDelete = useCallback(
    async (name: string) => {
      try {
        await deleteWorkspace(name);
        await loadWorkspaces();
      } catch {
        // ignore
      }
    },
    [loadWorkspaces]
  );

  const handleRestore = useCallback(
    (ws: WorkspaceSnapshot) => {
      onRestore(ws);
      onClose();
    },
    [onRestore, onClose]
  );

  const formatDate = (iso: string) => {
    if (!iso) return "";
    try {
      const d = new Date(iso);
      return d.toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return iso;
    }
  };

  const userWorkspaces = workspaces.filter((w) => !w.is_template && !w.is_recipe);
  const recipes = [...TASK_TEMPLATES, ...workspaces.filter((w) => w.is_recipe)];

  const startRecipeEdit = useCallback((recipe: WorkspaceSnapshot) => {
    setEditingRecipeName(recipe.name);
    setSaveName(recipe.name);
    setSaveAsRecipe(true);
    setRecipeDescription(recipe.recipe_description || "");
    setRecipeNextAction(recipe.recipe_next_action || "");
  }, []);

  return (
    <div className="workspace-mgr-overlay" onClick={onClose}>
      <div ref={modalRef} className="workspace-mgr-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="workspace-mgr-header">
          <div className="workspace-mgr-header-left">
            <Layers size={16} />
            <h3>Workspaces</h3>
          </div>
          <button onClick={onClose} title="Close">
            <X size={16} />
          </button>
        </div>

        {/* Save current */}
        <div className="workspace-mgr-save">
          <div className="workspace-mgr-save-fields">
            <input
              type="text"
              placeholder="Save current workspace as..."
              value={saveName}
              onChange={(e) => setSaveName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSave();
              }}
            />
            <div className="workspace-mgr-save-meta">
              <label className="workspace-mgr-recipe-toggle">
                <input
                  type="checkbox"
                  checked={saveAsRecipe}
                  onChange={(e) => setSaveAsRecipe(e.target.checked)}
                />
                Save as recipe
              </label>
              <span className="workspace-mgr-item-desc">
                {saveAsRecipe
                  ? "Recipes become reusable launch presets with guidance."
                  : "Save the current panel layout and reopen it later."}
              </span>
            </div>
            {saveAsRecipe && (
              <>
                <input
                  type="text"
                  placeholder="Recipe description"
                  value={recipeDescription}
                  onChange={(e) => setRecipeDescription(e.target.value)}
                />
                <input
                  type="text"
                  placeholder="Suggested next action"
                  value={recipeNextAction}
                  onChange={(e) => setRecipeNextAction(e.target.value)}
                />
              </>
            )}
          </div>
          <button
            onClick={handleSave}
            disabled={!saveName.trim() || saving}
            title={saveAsRecipe ? "Save recipe" : "Save workspace"}
          >
            <Save size={14} />
            {saving ? "Saving..." : editingRecipeName ? "Update" : "Save"}
          </button>
        </div>

        {/* Body */}
        <div className="workspace-mgr-body">
          {/* Task templates */}
          <div className="workspace-mgr-section">
            <div className="workspace-mgr-section-title">Recipes</div>
            {recipes.map((tmpl) => (
              <div key={tmpl.name} className="workspace-mgr-item template">
                <div className="workspace-mgr-item-info">
                  <span className="workspace-mgr-item-name">{tmpl.name}</span>
                  <span className="workspace-mgr-item-desc">
                    {describeSnapshot(tmpl)}
                  </span>
                  {(tmpl.recipe_description || tmpl.recipe_next_action) && (
                    <span className="workspace-mgr-item-recipe-meta">
                      {[tmpl.recipe_description, tmpl.recipe_next_action].filter(Boolean).join(" • ")}
                    </span>
                  )}
                </div>
                <div className="workspace-mgr-item-actions">
                  <button
                    className="workspace-mgr-restore-btn"
                    onClick={() => handleRestore(tmpl)}
                    title="Launch recipe"
                  >
                    <Play size={13} /> Launch
                  </button>
                  {!tmpl.is_template && (
                    <>
                      <button
                        className="workspace-mgr-delete-btn"
                        onClick={() => startRecipeEdit(tmpl)}
                        title="Edit recipe"
                      >
                        <Pencil size={13} />
                      </button>
                      <button
                        className="workspace-mgr-delete-btn"
                        onClick={() => handleDelete(tmpl.name)}
                        title="Delete recipe"
                      >
                        <Trash2 size={13} />
                      </button>
                    </>
                  )}
                </div>
              </div>
            ))}
          </div>

          {/* Saved workspaces */}
          <div className="workspace-mgr-section">
            <div className="workspace-mgr-section-title">
              Saved Workspaces
              {activeWorkspaceName && (
                <span className="workspace-mgr-active-badge">
                  Active: {activeWorkspaceName}
                </span>
              )}
            </div>
            {loading ? (
              <div className="workspace-mgr-empty">Loading...</div>
            ) : userWorkspaces.length === 0 ? (
              <div className="workspace-mgr-empty">
                No saved workspaces yet. Use the field above to save the current layout.
              </div>
            ) : (
              userWorkspaces.map((ws) => (
                <div
                  key={ws.name}
                  className={`workspace-mgr-item ${ws.name === activeWorkspaceName ? "active" : ""}`}
                >
                  <div className="workspace-mgr-item-info">
                    <span className="workspace-mgr-item-name">{ws.name}</span>
                    <span className="workspace-mgr-item-desc">
                      {describeSnapshot(ws)}
                      {ws.created_at && ` \u2022 ${formatDate(ws.created_at)}`}
                    </span>
                  </div>
                  <div className="workspace-mgr-item-actions">
                    <button
                      className="workspace-mgr-restore-btn"
                      onClick={() => handleRestore(ws)}
                      title="Restore workspace"
                    >
                      <Play size={13} /> Restore
                    </button>
                    <button
                      className="workspace-mgr-delete-btn"
                      onClick={() => handleDelete(ws.name)}
                      title="Delete workspace"
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="workspace-mgr-footer">
          <span>
            {userWorkspaces.length} saved workspace{userWorkspaces.length !== 1 ? "s" : ""}
          </span>
          <kbd>Cmd+Shift+L</kbd>
        </div>
      </div>
    </div>
  );
}

/** Human-readable summary of what a snapshot configures. */
function describeSnapshot(ws: WorkspaceSnapshot): string {
  const parts: string[] = [];
  const panels = ws.panels;
  if (panels.sidebar) parts.push("Sidebar");
  if (panels.terminal) parts.push("Terminal");
  if (panels.editor) parts.push("Editor");
  if (panels.git) parts.push("Git");
  if (ws.open_files.length > 0)
    parts.push(`${ws.open_files.length} file${ws.open_files.length > 1 ? "s" : ""}`);
  if (ws.git_branch) parts.push(`branch: ${ws.git_branch}`);
  if (ws.is_recipe) parts.push("recipe");
  return parts.join(" + ") || "Empty";
}
