import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { Brain, Pencil, Plus, Save, Trash2, X } from "lucide-react";
import {
  createPersonalMemory,
  deletePersonalMemory,
  fetchPersonalMemory,
  updatePersonalMemory,
} from "../api";
import type { MemoryScope, PersonalMemoryItem, ProjectInfo } from "../api";
import { formatScope, describeScope, formatRelativeDate } from "./helpers";

const SCOPE_OPTIONS: MemoryScope[] = ["global", "project", "session"];

interface Props {
  onClose: () => void;
  projects: ProjectInfo[];
  activeProjectIndex: number;
  activeSessionId: string | null;
}

export function MemoryModal({
  onClose,
  projects,
  activeProjectIndex,
  activeSessionId,
}: Props) {
  const [items, setItems] = useState<PersonalMemoryItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [label, setLabel] = useState("");
  const [content, setContent] = useState("");
  const [scope, setScope] = useState<MemoryScope>("global");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingLabel, setEditingLabel] = useState("");
  const [editingContent, setEditingContent] = useState("");
  const [editingScope, setEditingScope] = useState<MemoryScope>("global");
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  const loadMemory = useCallback(async () => {
    try {
      const resp = await fetchPersonalMemory();
      setItems((resp?.memory ?? []).filter(Boolean));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadMemory(); }, [loadMemory]);

  const grouped = useMemo(() => {
    return SCOPE_OPTIONS.map((current) => ({
      scope: current,
      items: items.filter((item) => item.scope === current),
    }));
  }, [items]);

  const handleCreate = useCallback(async () => {
    if (!label.trim() || !content.trim()) return;
    setSaving(true);
    try {
      const created = await createPersonalMemory({
        label: label.trim(),
        content: content.trim(),
        scope,
        project_index: scope === "project" || scope === "session" ? activeProjectIndex : null,
        session_id: scope === "session" ? activeSessionId : null,
      });
      setItems((prev) => [created, ...prev]);
      setLabel("");
      setContent("");
      setScope("global");
    } finally {
      setSaving(false);
    }
  }, [label, content, scope, activeProjectIndex, activeSessionId]);

  const handleDelete = useCallback(async (id: string) => {
    await deletePersonalMemory(id);
    setItems((prev) => prev.filter((item) => item.id !== id));
  }, []);

  const startEdit = useCallback((item: PersonalMemoryItem) => {
    setEditingId(item.id);
    setEditingLabel(item.label);
    setEditingContent(item.content);
    setEditingScope(item.scope);
  }, []);

  const cancelEdit = useCallback(() => {
    setEditingId(null);
    setEditingLabel("");
    setEditingContent("");
    setEditingScope("global");
  }, []);

  const handleSaveEdit = useCallback(async () => {
    if (!editingId || !editingLabel.trim() || !editingContent.trim()) return;
    const updated = await updatePersonalMemory(editingId, {
      label: editingLabel.trim(),
      content: editingContent.trim(),
      scope: editingScope,
      project_index: editingScope === "project" || editingScope === "session" ? activeProjectIndex : null,
      session_id: editingScope === "session" ? activeSessionId : null,
    });
    setItems((prev) => prev.map((item) => (item.id === updated.id ? updated : item)));
    cancelEdit();
  }, [editingId, editingLabel, editingContent, cancelEdit]);

  return (
    <div className="memory-overlay" onClick={onClose}>
      <div ref={modalRef} className="memory-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="memory-header">
          <div className="memory-header-left">
            <Brain size={16} />
            <h3>Personal Memory</h3>
            <span className="memory-count">{items.length}</span>
          </div>
          <button onClick={onClose} aria-label="Close memory"><X size={16} /></button>
        </div>

        <div className="memory-create">
          <div className="memory-create-grid">
            <input className="memory-input" value={label} onChange={(e) => setLabel(e.target.value)} placeholder="Memory label" />
            <select className="memory-select" value={scope} onChange={(e) => setScope(e.target.value as MemoryScope)}>
              {SCOPE_OPTIONS.map((option) => (
                <option key={option} value={option}>{formatScope(option)}</option>
              ))}
            </select>
          </div>
          <textarea className="memory-textarea" rows={3} value={content} onChange={(e) => setContent(e.target.value)} placeholder="Store a stable preference, recurring constraint, or working norm" />
          <div className="memory-create-footer">
            <span className="memory-context">
              {scope === "global"
                ? "Visible everywhere"
                : scope === "project"
                  ? `Applies to ${projects[activeProjectIndex]?.name ?? "current project"}`
                  : activeSessionId
                    ? `Applies to session ${activeSessionId.slice(0, 8)}`
                    : "No active session to scope memory"}
            </span>
            <button className="memory-create-btn" onClick={handleCreate} disabled={saving || !label.trim() || !content.trim() || (scope === "session" && !activeSessionId)}>
              <Plus size={14} />
              {saving ? "Saving..." : "Save memory"}
            </button>
          </div>
        </div>

        <div className="memory-body">
          {loading ? (
            <div className="memory-empty">Loading memory...</div>
          ) : items.length === 0 ? (
            <div className="memory-empty">No personal memory yet.</div>
          ) : (
            grouped.map(({ scope: currentScope, items: scopedItems }) =>
              scopedItems.length > 0 ? (
                <section key={currentScope} className="memory-section">
                  <div className="memory-section-title">{formatScope(currentScope)}</div>
                  {scopedItems.map((item) => (
                    <div key={item.id} className="memory-item">
                      <div className="memory-item-main">
                        {editingId === item.id ? (
                          <>
                            <input className="memory-input" value={editingLabel} onChange={(e) => setEditingLabel(e.target.value)} />
                            <select className="memory-select" value={editingScope} onChange={(e) => setEditingScope(e.target.value as MemoryScope)}>
                              {SCOPE_OPTIONS.map((option) => (<option key={option} value={option}>{formatScope(option)}</option>))}
                            </select>
                            <textarea className="memory-textarea" rows={3} value={editingContent} onChange={(e) => setEditingContent(e.target.value)} />
                          </>
                        ) : (
                          <>
                            <div className="memory-item-label">{item.label}</div>
                            <div className="memory-item-content">{item.content}</div>
                          </>
                        )}
                        <div className="memory-item-meta">
                          <span>{describeScope(item, projects)}</span>
                          <span>updated {formatRelativeDate(item.updated_at)}</span>
                        </div>
                      </div>
                      <div className="memory-item-actions">
                        {editingId === item.id ? (
                          <>
                            <button className="memory-edit-btn" onClick={handleSaveEdit} aria-label={`Save ${item.label}`}><Save size={14} /></button>
                            <button className="memory-delete-btn" onClick={cancelEdit} aria-label={`Cancel edit for ${item.label}`}><X size={14} /></button>
                          </>
                        ) : (
                          <>
                            <button className="memory-edit-btn" onClick={() => startEdit(item)} aria-label={`Edit ${item.label}`}><Pencil size={14} /></button>
                            <button className="memory-delete-btn" onClick={() => handleDelete(item.id)} aria-label={`Delete ${item.label}`}><Trash2 size={14} /></button>
                          </>
                        )}
                      </div>
                    </div>
                  ))}
                </section>
              ) : null
            )
          )}
        </div>
      </div>
    </div>
  );
}
