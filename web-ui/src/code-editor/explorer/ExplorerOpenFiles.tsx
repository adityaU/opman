/**
 * ExplorerOpenFiles — the strip of files currently open in the editor.
 *
 * This is a different kind of list from the tree: short, flat, and about
 * session state rather than the project. It reads as a rail rather than a
 * second tree, and the close control holds its space at all times so the row
 * never reflows under the cursor on hover.
 */
import { X } from "lucide-react";
import { FileTile } from "./ExplorerBits";
import type { OpenFileEntry } from "../types";

interface Props {
  openFiles: OpenFileEntry[];
  activeFilePath: string | null;
  onSelect: (file: OpenFileEntry) => void;
  onClose: (path: string) => void;
}

export function ExplorerOpenFiles({ openFiles, activeFilePath, onSelect, onClose }: Props) {
  if (openFiles.length === 0) return null;
  const dirty = openFiles.filter((f) => f.editedContent !== null).length;

  return (
    <div className="xpl-open">
      <div className="xpl-open-head">
        <span>Open</span>
        <span className="xpl-open-count">
          {openFiles.length}
          {dirty > 0 && <span className="xpl-open-dirty" title={`${dirty} unsaved`} />}
        </span>
      </div>
      <div className="xpl-open-list">
        {openFiles.map((file) => {
          const name = file.path.split("/").pop() || file.path;
          const isActive = file.path === activeFilePath;
          const isDirty = file.editedContent !== null;
          return (
            <div
              key={file.path}
              className={`xpl-open-item${isActive ? " is-active" : ""}`}
              title={file.path}
            >
              <button type="button" className="xpl-open-btn" onClick={() => onSelect(file)}>
                <FileTile name={name} />
                <span className="xpl-name">{name}</span>
              </button>
              <button
                type="button"
                className={`xpl-open-close${isDirty ? " is-dirty" : ""}`}
                aria-label={isDirty ? `Close ${name} — unsaved changes` : `Close ${name}`}
                onClick={() => onClose(file.path)}
              >
                <X size={11} />
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
