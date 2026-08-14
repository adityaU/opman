/**
 * The sidebar's standing preference: whether it is showing, and how wide.
 *
 * The workspace already remembers every pane across a reload, so a sidebar that
 * springs back open reads as the one part of the desk that forgot. Stored and
 * read exactly like the density preference in ./density — one localStorage key,
 * a total parser, and a write that stays quiet when storage refuses.
 */

export interface SidebarPrefs {
  readonly open: boolean;
  readonly width: number;
}

const SIDEBAR_KEY = "opman.sidebar";

/** The resize bounds live here so a stored width is clamped to what the drag handle allows. */
export const SIDEBAR_MIN_WIDTH = 200;
export const SIDEBAR_MAX_WIDTH = 500;

export const DEFAULT_SIDEBAR_PREFS: SidebarPrefs = { open: true, width: 280 };

function parse(value: unknown, fallback: SidebarPrefs): SidebarPrefs {
  if (typeof value !== "object" || value === null) return fallback;
  const record = value as Record<string, unknown>;
  const width = record.width;
  return {
    open: typeof record.open === "boolean" ? record.open : fallback.open,
    width:
      typeof width === "number" && Number.isFinite(width)
        ? Math.min(Math.max(width, SIDEBAR_MIN_WIDTH), SIDEBAR_MAX_WIDTH)
        : fallback.width,
  };
}

export function loadSidebarPrefs(
  fallback: SidebarPrefs = DEFAULT_SIDEBAR_PREFS,
  storage: Pick<Storage, "getItem"> = localStorage,
): SidebarPrefs {
  try {
    const text = storage.getItem(SIDEBAR_KEY);
    if (!text) return fallback;
    return parse(JSON.parse(text), fallback);
  } catch {
    return fallback;
  }
}

export function persistSidebarPrefs(
  prefs: SidebarPrefs,
  storage: Pick<Storage, "setItem"> = localStorage,
): void {
  try {
    storage.setItem(SIDEBAR_KEY, JSON.stringify(prefs));
  } catch {
    /* private mode or a full quota — this session still works, it just will not survive */
  }
}
