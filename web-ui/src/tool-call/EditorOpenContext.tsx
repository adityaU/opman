import React, { createContext, useContext } from "react";

/** Opens a file in the code-editor panel. `line` is 1-based, optional. */
export type OpenFileInEditor = (path: string, line?: number | null) => void;

const EditorOpenContext = createContext<OpenFileInEditor | null>(null);

export const EditorOpenProvider = EditorOpenContext.Provider;

/** Returns the file opener if a provider is mounted, else null. */
export function useOpenFileInEditor(): OpenFileInEditor | null {
  return useContext(EditorOpenContext);
}
