import React, { useState, useCallback, useEffect, useRef } from "react";
import { deleteSession, renameSession, removeProject } from "../api";

// ═══════════════════════════════════════════════════════
//  useDeleteSession
// ═══════════════════════════════════════════════════════

export function useDeleteSession() {
  const [deleteConfirm, setDeleteConfirm] = useState<{
    sessionId: string;
    sessionTitle: string;
  } | null>(null);
  const [deleteLoading, setDeleteLoading] = useState(false);

  const handleDelete = useCallback(async () => {
    if (!deleteConfirm) return;
    setDeleteLoading(true);
    try {
      await deleteSession(deleteConfirm.sessionId);
    } catch (err) {
      console.error("Failed to delete session:", err);
    } finally {
      setDeleteLoading(false);
      setDeleteConfirm(null);
    }
  }, [deleteConfirm]);

  return { deleteConfirm, setDeleteConfirm, deleteLoading, handleDelete } as const;
}

// ═══════════════════════════════════════════════════════
//  useRenameSession
// ═══════════════════════════════════════════════════════

export function useRenameSession() {
  const [renameTarget, setRenameTarget] = useState<{
    sessionId: string;
    currentTitle: string;
  } | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [renameLoading, setRenameLoading] = useState(false);
  const renameInputRef = useRef<HTMLInputElement>(null) as React.RefObject<HTMLInputElement>;

  // Focus rename input when it appears
  useEffect(() => {
    if (renameTarget && renameInputRef.current) {
      renameInputRef.current.focus();
      renameInputRef.current.select();
    }
  }, [renameTarget]);

  const handleRenameSubmit = useCallback(async () => {
    if (!renameTarget || !renameValue.trim() || renameLoading) return;
    const trimmed = renameValue.trim();
    if (trimmed === renameTarget.currentTitle) {
      setRenameTarget(null);
      return;
    }
    setRenameLoading(true);
    try {
      await renameSession(renameTarget.sessionId, trimmed);
    } catch (err) {
      console.error("Failed to rename session:", err);
    } finally {
      setRenameLoading(false);
      setRenameTarget(null);
    }
  }, [renameTarget, renameValue, renameLoading]);

  const handleRenameKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleRenameSubmit();
      } else if (e.key === "Escape") {
        setRenameTarget(null);
      }
    },
    [handleRenameSubmit],
  );

  return {
    renameTarget,
    setRenameTarget,
    renameValue,
    setRenameValue,
    renameLoading,
    renameInputRef,
    handleRenameSubmit,
    handleRenameKeyDown,
  } as const;
}

// ═══════════════════════════════════════════════════════
//  useRemoveProject
// ═══════════════════════════════════════════════════════

export function useRemoveProject() {
  const [removeConfirm, setRemoveConfirm] = useState<{
    index: number;
    name: string;
  } | null>(null);
  const [removeLoading, setRemoveLoading] = useState(false);

  const handleRemoveProject = useCallback(async () => {
    if (!removeConfirm || removeLoading) return;
    setRemoveLoading(true);
    try {
      await removeProject(removeConfirm.index);
    } catch (err) {
      console.error("Failed to remove project:", err);
    } finally {
      setRemoveLoading(false);
      setRemoveConfirm(null);
    }
  }, [removeConfirm, removeLoading]);

  return { removeConfirm, setRemoveConfirm, removeLoading, handleRemoveProject } as const;
}

// ═══════════════════════════════════════════════════════
//  DeleteSessionModal
// ═══════════════════════════════════════════════════════

interface DeleteSessionModalProps {
  confirm: { sessionId: string; sessionTitle: string };
  loading: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

export function DeleteSessionModal({ confirm, loading, onClose, onConfirm }: DeleteSessionModalProps) {
  return (
    <div className="sb-modal-overlay" onClick={() => !loading && onClose()}>
      <div className="sb-modal" onClick={(e) => e.stopPropagation()}>
        <div className="sb-modal-title">Delete Session</div>
        <div className="sb-modal-body">
          Are you sure you want to delete{" "}
          <strong>{confirm.sessionTitle || confirm.sessionId.slice(0, 12)}</strong>?
          This action cannot be undone.
        </div>
        <div className="sb-modal-actions">
          <button
            className="sb-modal-btn sb-modal-cancel"
            onClick={onClose}
            disabled={loading}
          >
            Cancel
          </button>
          <button
            className="sb-modal-btn sb-modal-danger"
            onClick={onConfirm}
            disabled={loading}
          >
            {loading ? "Deleting..." : "Delete"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════
//  RemoveProjectModal
// ═══════════════════════════════════════════════════════

interface RemoveProjectModalProps {
  confirm: { index: number; name: string };
  loading: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

export function RemoveProjectModal({ confirm, loading, onClose, onConfirm }: RemoveProjectModalProps) {
  return (
    <div className="sb-modal-overlay" onClick={() => !loading && onClose()}>
      <div className="sb-modal" onClick={(e) => e.stopPropagation()}>
        <div className="sb-modal-title">Remove Project</div>
        <div className="sb-modal-body">
          Are you sure you want to remove{" "}
          <strong>{confirm.name}</strong> from your projects?
          This will not delete any files.
        </div>
        <div className="sb-modal-actions">
          <button
            className="sb-modal-btn sb-modal-cancel"
            onClick={onClose}
            disabled={loading}
          >
            Cancel
          </button>
          <button
            className="sb-modal-btn sb-modal-danger"
            onClick={onConfirm}
            disabled={loading}
          >
            {loading ? "Removing..." : "Remove"}
          </button>
        </div>
      </div>
    </div>
  );
}
