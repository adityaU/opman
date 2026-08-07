/**
 * ExplorerResults — what the tree becomes while the filter has a query.
 *
 * A filtered tree that keeps its nesting makes you read hierarchy you did not
 * ask about. Results are a flat ranked list instead: the filename leads at
 * full contrast with the matched span marked, and the folder it lives in
 * trails behind it, quiet, so you can tell two same-named files apart without
 * it competing with the name.
 */
import { Folder } from "lucide-react";
import { FileTile, MatchedName } from "./ExplorerBits";
import type { FileSearchEntry } from "../../api";

interface Props {
  query: string;
  results: FileSearchEntry[];
  searching: boolean;
  error: string | null;
  activeFilePath: string | null;
  onOpenFile: (path: string) => void;
  onOpenDir: (path: string) => void;
}

export function ExplorerResults({
  query, results, searching, error, activeFilePath, onOpenFile, onOpenDir,
}: Props) {
  if (error) {
    return <div className="xpl-state xpl-state-error">{error}</div>;
  }

  if (results.length === 0) {
    if (searching) return <div className="xpl-state">Searching…</div>;
    return (
      <div className="xpl-state">
        No file matches <strong>{query}</strong>.
        <span className="xpl-state-hint">Try part of the name, or a folder it sits in.</span>
      </div>
    );
  }

  return (
    <div className="xpl-results" role="listbox" aria-label="Search results">
      <div className="xpl-results-count">
        {results.length} {results.length === 1 ? "match" : "matches"}
      </div>
      {results.map((entry, index) => {
        const parent = entry.path.slice(0, Math.max(0, entry.path.length - entry.name.length - 1));
        return (
          <button
            key={entry.path}
            type="button"
            role="option"
            aria-selected={entry.path === activeFilePath}
            className={`xpl-result${entry.path === activeFilePath ? " is-active" : ""}`}
            style={{ "--i": Math.min(index, 12) } as React.CSSProperties}
            title={entry.path}
            onClick={() => (entry.is_dir ? onOpenDir(entry.path) : onOpenFile(entry.path))}
          >
            {entry.is_dir
              ? <span className="xpl-result-folder" aria-hidden="true"><Folder size={13} /></span>
              : <FileTile name={entry.name} />}
            <span className="xpl-result-text">
              <span className="xpl-result-name">
                <MatchedName name={entry.name} query={query} />
              </span>
              {parent && <span className="xpl-result-path">{parent}</span>}
            </span>
          </button>
        );
      })}
    </div>
  );
}
