import React from "react";
import { Folder, Plus, Star } from "lucide-react";
import type { DirEntry } from "./api";

interface Props {
  entries: DirEntry[];
  selected: number;
  loading: boolean;
  browseLoading: boolean;
  filter: string;
  listRef: React.RefObject<HTMLDivElement>;
  onBrowse: (path: string) => void;
  onAdd: (entry: DirEntry) => void;
  onSelect: (index: number) => void;
}

export function DirectoryEntryList({
  entries,
  selected,
  loading,
  browseLoading,
  filter,
  listRef,
  onBrowse,
  onAdd,
  onSelect,
}: Props) {
  if (browseLoading) {
    return <div className="add-project-body" ref={listRef}><div className="add-project-empty">Loading...</div></div>;
  }
  if (entries.length === 0) {
    return (
      <div className="add-project-body" ref={listRef}>
        <div className="add-project-empty">
          {filter ? "No matching directories" : "No subdirectories"}
        </div>
      </div>
    );
  }

  return (
    <div className="add-project-body" ref={listRef}>
      {entries.map((entry, index) => (
        <div
          key={entry.path}
          className={`add-project-entry${index === selected ? " add-project-entry-selected" : ""}${entry.is_project ? " add-project-entry-existing" : ""}`}
          onClick={() => onBrowse(entry.path)}
          onDoubleClick={() => onAdd(entry)}
          onMouseEnter={() => onSelect(index)}
          title={entry.is_project ? `${entry.path} (already added)` : entry.path}
        >
          <Folder size={14} className="add-project-entry-icon" />
          <span className="add-project-entry-name">{entry.name}</span>
          {entry.is_project && <Star size={11} className="add-project-entry-star" />}
          <button
            className="add-project-entry-add"
            onClick={(event) => {
              event.stopPropagation();
              onAdd(entry);
            }}
            title={entry.is_project ? "Already added" : "Add as project"}
            disabled={entry.is_project || loading}
          >
            {entry.is_project ? <Star size={11} /> : <Plus size={13} />}
          </button>
        </div>
      ))}
    </div>
  );
}
