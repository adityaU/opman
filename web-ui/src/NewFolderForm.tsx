import React from "react";
import { FolderPlus } from "lucide-react";

interface Props {
  name: string;
  loading: boolean;
  onNameChange: (name: string) => void;
  onSubmit: (event: React.FormEvent) => void;
  onCancel: () => void;
}

export function NewFolderForm({ name, loading, onNameChange, onSubmit, onCancel }: Props) {
  return (
    <form className="add-project-new-folder-form" onSubmit={onSubmit}>
      <FolderPlus size={14} className="add-project-new-folder-icon" />
      <input
        className="add-project-new-folder-input"
        value={name}
        onChange={(event) => onNameChange(event.target.value)}
        placeholder="Folder name"
        autoFocus
        disabled={loading}
      />
      <button className="add-project-new-folder-submit" type="submit" disabled={loading || !name.trim()}>
        {loading ? "Creating..." : "Create"}
      </button>
      <button className="add-project-new-folder-cancel" type="button" onClick={onCancel} disabled={loading}>
        Cancel
      </button>
    </form>
  );
}
