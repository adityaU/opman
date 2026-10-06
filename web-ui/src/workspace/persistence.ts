/**
 * Workspace persistence.
 *
 * The layout is the user's desk: losing it to a reload, a bad write or a shape
 * change is worse than losing any single thing on it. So the parser is total —
 * it validates structurally and returns `null` rather than throwing — and a
 * partially-recognised tree is repaired toward something usable instead of
 * discarded.
 *
 * A pane's trail is persisted along with the widget it is showing, so "the file
 * I had open before this one" survives a reload rather than only a window
 * switch. Everything it points at is either server-side and outlives the tab —
 * shells, chat sessions, browser tabs — or is a path, so an entry restored is an
 * entry that still means something. The one exception is a shell that has since
 * exited, and the terminal panel already checks the server before attaching.
 */

import { EMPTY_HISTORY, repairHistory, type PaneHistory } from "./history";
import { emptyWorkspace } from "./reducer";
import { normalize } from "./tree";
import { isRecord, parseWidget } from "./parseWidget";
import {
  asPaneId,
  asSplitId,
  asWindowId,
  DEFAULT_CHROME,
  type ChromeState,
  type Node,
  type PaneId,
  type WidgetState,
  type Workspace,
  type WorkspaceWindow,
} from "./types";
import { storageKey } from "../api/base";

const STORAGE_KEY = storageKey("opman.workspace");
const VERSION = 1;

interface Envelope {
  readonly version: number;
  readonly workspace: unknown;
}

// ── Read ────────────────────────────────────────────────

export function loadWorkspace(storage: Pick<Storage, "getItem"> = localStorage): Workspace {
  const raw = read(storage);
  if (!raw) return emptyWorkspace();
  return parseWorkspace(migrate(raw)) ?? emptyWorkspace();
}

function read(storage: Pick<Storage, "getItem">): Envelope | null {
  try {
    const text = storage.getItem(STORAGE_KEY);
    if (!text) return null;
    const parsed: unknown = JSON.parse(text);
    if (!isRecord(parsed) || typeof parsed.version !== "number") return null;
    return { version: parsed.version, workspace: parsed.workspace };
  } catch {
    return null;
  }
}

/**
 * Bring an older envelope up to the current version. There is only one version
 * today; the seam exists so the next shape change is a function rather than a
 * decision about whether to throw everyone's layout away.
 */
function migrate(envelope: Envelope): unknown {
  if (envelope.version > VERSION) return null;
  return envelope.workspace;
}

// ── Write ───────────────────────────────────────────────

export function saveWorkspace(
  workspace: Workspace,
  storage: Pick<Storage, "setItem"> = localStorage,
): void {
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify({ version: VERSION, workspace }));
  } catch {
    // Private browsing and full quotas both land here. A workspace that cannot
    // be saved still works for this session, so there is nothing to report.
  }
}

// ── Parsing ─────────────────────────────────────────────

function parseWorkspace(value: unknown): Workspace | null {
  if (!isRecord(value) || !Array.isArray(value.windows)) return null;
  const windows = value.windows.map(parseWindow).filter((w): w is WorkspaceWindow => w !== null);
  if (windows.length === 0) return null;

  const active = typeof value.activeWindowId === "string" ? asWindowId(value.activeWindowId) : null;
  return {
    windows,
    activeWindowId: active && windows.some((w) => w.id === active) ? active : windows[0].id,
    chrome: parseChrome(value.chrome),
  };
}

function parseChrome(value: unknown): ChromeState {
  if (!isRecord(value)) return DEFAULT_CHROME;
  const flag = (key: keyof ChromeState) =>
    typeof value[key] === "boolean" ? (value[key] as boolean) : DEFAULT_CHROME[key];
  // Zen is deliberately not restored. `rail` is a standing preference; Zen is
  // where you happened to be when the tab closed, and coming back to a
  // chromeless shell you did not ask for reads as a bug.
  return { rail: flag("rail"), zen: false };
}

function parseWindow(value: unknown): WorkspaceWindow | null {
  if (!isRecord(value) || typeof value.id !== "string") return null;
  const root = parseNode(value.root);
  if (!root) return null;

  const panes = collectPaneIds(root);
  const focused = typeof value.focusedPaneId === "string" ? asPaneId(value.focusedPaneId) : null;
  const zoomed = typeof value.zoomedPaneId === "string" ? asPaneId(value.zoomedPaneId) : null;

  return {
    id: asWindowId(value.id),
    name: typeof value.name === "string" && value.name ? value.name : "1",
    root,
    focusedPaneId: focused && panes.has(focused) ? focused : [...panes][0],
    zoomedPaneId: zoomed && panes.has(zoomed) ? zoomed : null,
  };
}

function collectPaneIds(node: Node): Set<PaneId> {
  if (node.type === "leaf") return new Set([node.id]);
  return new Set(node.children.flatMap((child) => [...collectPaneIds(child)]));
}

/**
 * Parse a node, dropping children that fail. A split left with one usable
 * child collapses into it and one left with none fails upward — so a corrupt
 * branch costs that branch, never the whole desk.
 */
function parseNode(value: unknown): Node | null {
  if (!isRecord(value) || typeof value.id !== "string") return null;

  if (value.type === "leaf") {
    const id = asPaneId(value.id);
    const widget = parseWidget(value.widget, id);
    // Repaired against the widget rather than trusted: the two are stored side
    // by side and a half-finished write, an older layout with no trail at all,
    // or a hand-edited value could leave them disagreeing. The widget wins,
    // because the widget is what the pane will render.
    return { type: "leaf", id, widget, history: repairHistory(parseHistory(value.history, id), widget) };
  }
  if (value.type !== "split" || !Array.isArray(value.children)) return null;
  if (value.dir !== "row" && value.dir !== "col") return null;

  const rawSizes = Array.isArray(value.sizes) ? value.sizes : [];
  const kept = value.children
    .map((child, index) => ({ node: parseNode(child), size: rawSizes[index] }))
    .filter((entry): entry is { node: Node; size: unknown } => entry.node !== null);

  if (kept.length === 0) return null;
  if (kept.length === 1) return kept[0].node;

  const sizes = kept.map((entry) => (typeof entry.size === "number" && entry.size > 0 ? entry.size : 1));
  return {
    type: "split",
    id: asSplitId(value.id),
    dir: value.dir,
    children: kept.map((entry) => entry.node),
    sizes: normalize(sizes),
  };
}

/**
 * A pane's trail.
 *
 * Entries that no longer parse are dropped rather than failing the pane, so one
 * unreadable past target costs that entry and not the desk. The cursor is
 * clamped into the surviving list — including onto `entries.length`, which is
 * how "showing nothing" is spelled — and `repairHistory` has the final say once
 * the widget is known.
 */
function parseHistory(value: unknown, paneId: PaneId): PaneHistory {
  if (!isRecord(value) || !Array.isArray(value.entries)) return EMPTY_HISTORY;
  const entries = value.entries
    .map((entry) => parseWidget(entry, paneId))
    .filter((entry): entry is WidgetState => entry !== null);
  const raw = typeof value.index === "number" ? Math.trunc(value.index) : entries.length;
  return { entries, index: Math.min(Math.max(raw, 0), entries.length) };
}
