import { useCallback, useEffect, useState } from "react";

/**
 * The shell a terminal without a pane came back to, remembered per project.
 *
 * The desktop workspace stores this on the widget, because a pane's shell is
 * part of its layout. Mobile has one terminal sheet and no layout to put it in,
 * so it keeps the choice here instead — otherwise reopening the sheet would ask
 * which shell every time, including when there is only ever one.
 *
 * Only a pointer is stored. If the shell has exited by the next visit the panel
 * finds that out from the server and shows the picker, which is the right
 * answer — a finished build should not look like a fresh prompt.
 */

const KEY = "opman-term-shell";

function read(): Record<string, string> {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    return isStringMap(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function isStringMap(value: unknown): value is Record<string, string> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  return Object.values(value).every((entry) => typeof entry === "string");
}

export function useRememberedShell(
  projectPath: string | null,
): readonly [string | null, (ptyId: string | null) => void] {
  const [ptyId, setPtyId] = useState<string | null>(null);

  useEffect(() => {
    setPtyId(projectPath ? read()[projectPath] ?? null : null);
  }, [projectPath]);

  const remember = useCallback(
    (next: string | null) => {
      setPtyId(next);
      if (!projectPath) return;
      try {
        const all = read();
        if (next) all[projectPath] = next;
        else delete all[projectPath];
        localStorage.setItem(KEY, JSON.stringify(all));
      } catch {
        // A full or blocked store costs the memory, not the terminal.
      }
    },
    [projectPath],
  );

  return [ptyId, remember];
}
