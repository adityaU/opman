import React, { createContext, useContext } from "react";

/** Opens a file in the code-editor panel. `line` is 1-based, optional. */
export type OpenFileInEditor = (path: string, line?: number | null) => void;

/**
 * Two ways to reveal a file, because there are two different intents.
 *
 * `open` is the answer when the caller is outside the editor — a tool card, an
 * MCP event — and the workspace picks a sensible pane. `openWhere` is for a
 * caller already inside the editor, where "beside this" and "instead of this"
 * are both reasonable and only the reader knows which; it defers to the same
 * pane-picking overlay a session click uses.
 */
export interface EditorOpener {
  readonly open: OpenFileInEditor;
  /** `label` names what is being placed, e.g. `Definition · parseConfig`. */
  readonly openWhere: (path: string, line: number | null, label: string) => void;
}

const EditorOpenContext = createContext<EditorOpener | null>(null);

export const EditorOpenProvider = EditorOpenContext.Provider;

/** Returns the file opener if a provider is mounted, else null. */
export function useOpenFileInEditor(): OpenFileInEditor | null {
  return useContext(EditorOpenContext)?.open ?? null;
}

/** The full opener, for callers that can offer a choice of pane. */
export function useEditorOpener(): EditorOpener | null {
  return useContext(EditorOpenContext);
}
