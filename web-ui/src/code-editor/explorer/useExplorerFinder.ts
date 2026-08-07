/**
 * useExplorerFinder — project-wide fuzzy file search behind the explorer's
 * filter field.
 *
 * Filtering only the directories that happen to be expanded is a filter that
 * lies: the file you are looking for is almost always in a folder you have not
 * opened. This hits the server's fuzzy index instead, so one field reaches the
 * whole project.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { searchFiles, type FileSearchEntry } from "../../api";

const DEBOUNCE_MS = 130;
const LIMIT = 60;

export interface FinderState {
  query: string;
  setQuery: (value: string) => void;
  clear: () => void;
  results: FileSearchEntry[];
  searching: boolean;
  error: string | null;
  /** True once the query is long enough for results to be meaningful. */
  active: boolean;
}

export function useExplorerFinder(): FinderState {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<FileSearchEntry[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const seq = useRef(0);

  const trimmed = query.trim();
  const active = trimmed.length > 0;

  useEffect(() => {
    if (!active) {
      seq.current += 1;
      setResults([]);
      setSearching(false);
      setError(null);
      return;
    }

    const ticket = (seq.current += 1);
    setSearching(true);
    const timer = setTimeout(() => {
      searchFiles(trimmed, LIMIT)
        .then((entries) => {
          if (seq.current !== ticket) return;
          setResults(entries);
          setError(null);
        })
        .catch(() => {
          if (seq.current !== ticket) return;
          setResults([]);
          setError("Search is unavailable right now.");
        })
        .finally(() => {
          if (seq.current === ticket) setSearching(false);
        });
    }, DEBOUNCE_MS);

    return () => clearTimeout(timer);
  }, [trimmed, active]);

  const clear = useCallback(() => setQuery(""), []);

  return { query, setQuery, clear, results, searching, error, active };
}
