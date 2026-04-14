import { useState, useRef, useCallback, useEffect } from "react";
import { searchFiles, readFile, type FileSearchEntry } from "../api";

/** A file that the user has @-mentioned for context injection. */
export interface FileMention {
  path: string;
  name: string;
  is_dir: boolean;
}

/**
 * Hook managing @file mention state: fuzzy search, selection, content fetching on send.
 *
 * The @ popover is shared with agents — this hook handles the file-search portion.
 * Parent coordinates which section (@agent vs @file) is shown.
 */
export function useFileMention() {
  const [fileMentions, setFileMentions] = useState<FileMention[]>([]);
  const [fileResults, setFileResults] = useState<FileSearchEntry[]>([]);
  const [fileLoading, setFileLoading] = useState(false);

  // Debounce timer ref
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Track latest query to discard stale responses
  const latestQueryRef = useRef("");

  /** Search files with debounce. Called on every @ filter keystroke. */
  const searchFilesDebounced = useCallback((query: string) => {
    latestQueryRef.current = query;

    if (debounceRef.current) clearTimeout(debounceRef.current);

    if (!query.trim()) {
      setFileResults([]);
      setFileLoading(false);
      return;
    }

    setFileLoading(true);
    debounceRef.current = setTimeout(async () => {
      try {
        const results = await searchFiles(query, 15);
        // Only apply if still the latest query
        if (latestQueryRef.current === query) {
          setFileResults(results);
        }
      } catch {
        if (latestQueryRef.current === query) setFileResults([]);
      } finally {
        if (latestQueryRef.current === query) setFileLoading(false);
      }
    }, 150);
  }, []);

  /** Add a file mention (no duplicates). */
  const addFileMention = useCallback((entry: FileSearchEntry) => {
    setFileMentions((prev) => {
      if (prev.some((m) => m.path === entry.path)) return prev;
      return [...prev, { path: entry.path, name: entry.name, is_dir: entry.is_dir }];
    });
  }, []);

  /** Remove a file mention by path. */
  const removeFileMention = useCallback((path: string) => {
    setFileMentions((prev) => prev.filter((m) => m.path !== path));
  }, []);

  /** Clear all file mentions (on send or session change). */
  const clearFileMentions = useCallback(() => {
    setFileMentions([]);
    setFileResults([]);
  }, []);

  /** Reset search results (when popover closes). */
  const clearFileResults = useCallback(() => {
    setFileResults([]);
    setFileLoading(false);
    latestQueryRef.current = "";
    if (debounceRef.current) clearTimeout(debounceRef.current);
  }, []);

  // Cleanup debounce on unmount
  useEffect(() => {
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, []);

  /**
   * Build context string from an explicit list of mentions.
   * Used when state has already been cleared (optimistic input reset).
   */
  const buildFileContextFrom = useCallback(async (mentions: FileMention[]): Promise<string> => {
    if (mentions.length === 0) return "";
    const reads = mentions.filter((m) => !m.is_dir).map(async (m) => {
      try {
        const resp = await readFile(m.path);
        return { path: m.path, content: resp.content };
      } catch {
        return { path: m.path, content: "[failed to read file]" };
      }
    });
    const results = await Promise.all(reads);
    const parts = results.map((r) => `<file path="${r.path}">\n${r.content}\n</file>`);
    for (const m of mentions.filter((m) => m.is_dir)) {
      parts.push(`<file path="${m.path}">[directory]</file>`);
    }
    return parts.length > 0 ? parts.join("\n\n") + "\n\n" : "";
  }, []);

  /** Build context string from current state (convenience wrapper). */
  const buildFileContext = useCallback(
    () => buildFileContextFrom(fileMentions),
    [fileMentions, buildFileContextFrom],
  );

  return {
    fileMentions,
    fileResults,
    fileLoading,
    searchFilesDebounced,
    addFileMention,
    removeFileMention,
    clearFileMentions,
    clearFileResults,
    buildFileContext,
    buildFileContextFrom,
  };
}
