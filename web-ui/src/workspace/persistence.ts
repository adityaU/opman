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

import { browserIdForProject } from "../api/browser";
import { EMPTY_HISTORY, repairHistory, type PaneHistory } from "./history";
import { emptyWorkspace } from "./reducer";
import { normalize } from "./tree";
import {
  asPaneId,
  asSplitId,
  asWindowId,
  DEFAULT_CHROME,
  WIDGET_KINDS,
  type ChromeState,
  type FileOpenRequest,
  type Node,
  type PaneId,
  type PaneEngine,
  type WidgetKind,
  type WidgetState,
  type Workspace,
  type WorkspaceWindow,
} from "./types";

const STORAGE_KEY = "opman.workspace";
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

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
 * A pane's engine, or null to follow the shell's.
 *
 * A runner name is the one field that cannot be defaulted — without it there is
 * no catalogue for the rest to mean anything in — so an entry missing it is
 * read as "never configured" rather than half-restored onto the wrong engine.
 */
function parseEngine(value: unknown): PaneEngine | null {
  if (!isRecord(value) || typeof value.runner !== "string" || !value.runner) return null;
  const model = isRecord(value.model)
    && typeof value.model.providerID === "string"
    && typeof value.model.modelID === "string"
      ? { providerID: value.model.providerID, modelID: value.model.modelID }
      : null;
  return {
    runner: value.runner,
    model,
    agent: typeof value.agent === "string" ? value.agent : "",
    effort: typeof value.effort === "string" ? value.effort : null,
    permission: typeof value.permission === "string" ? value.permission : "default",
  };
}

/**
 * The file a files pane was last asked to reveal. Without a path there is no
 * request, so a half-written entry restores as "no file" rather than as a jump
 * to nowhere.
 */
function parseFileOpen(value: unknown): FileOpenRequest | null {
  if (!isRecord(value) || typeof value.path !== "string" || !value.path) return null;
  return {
    path: value.path,
    line: typeof value.line === "number" ? value.line : null,
    seq: typeof value.seq === "number" ? value.seq : 0,
  };
}

/**
 * Which shell a terminal pane was showing.
 *
 * Layouts saved when a pane held a strip of tabs carry `ptyIds`; the pane now
 * shows one shell, so the first of them is restored and the rest are simply
 * left running — they are still in the picker, which is where they belonged all
 * along. An id whose shell has since exited resolves to the picker, because the
 * panel checks the server before attaching.
 */
function parsePtyId(value: Record<string, unknown>): string | null {
  if (typeof value.ptyId === "string" && value.ptyId) return value.ptyId;
  if (!Array.isArray(value.ptyIds)) return null;
  return value.ptyIds.find((id): id is string => typeof id === "string" && id !== "") ?? null;
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

function parseWidget(value: unknown, paneId: PaneId): WidgetState | null {
  if (!isRecord(value)) return null;
  const kind = value.kind;
  if (typeof kind !== "string" || !WIDGET_KINDS.includes(kind as WidgetKind)) return null;
  if (typeof value.projectPath !== "string") return null;
  const projectPath = value.projectPath;

  switch (kind as WidgetKind) {
    case "chat":
      return {
        kind: "chat",
        projectPath,
        sessionId: typeof value.sessionId === "string" ? value.sessionId : null,
        engine: parseEngine(value.engine),
      };
    case "files":
      return {
        kind: "files",
        projectPath,
        sessionId: typeof value.sessionId === "string" && value.sessionId ? value.sessionId : paneId,
        open: parseFileOpen(value.open),
      };
    case "terminal":
      return { kind: "terminal", projectPath, ptyId: parsePtyId(value) };
    case "git":
      return { kind: "git", projectPath };
    case "browser":
      return {
        kind: "browser",
        projectPath,
        // Browsers are per project, so a widget saved before `browserId` existed
        // — or one saved with a pane-scoped id — resolves to the project's
        // browser rather than being dropped or stranded on a tab nothing else
        // can reach.
        browserId:
          typeof value.browserId === "string" && value.browserId.startsWith("proj:")
            ? value.browserId
            : browserIdForProject(projectPath),
        url: typeof value.url === "string" && value.url ? value.url : null,
        // Restored as zero whatever it was. The counter only means "newer than
        // the last one this panel acted on", and after a reload the panel has
        // acted on nothing — so carrying the old value across would arm a
        // navigation the user did not ask for on first paint.
        reveal: 0,
      };
  }
}
