/**
 * Workspace persistence.
 *
 * The layout is the user's desk: losing it to a reload, a bad write or a shape
 * change is worse than losing any single thing on it. So the parser is total —
 * it validates structurally and returns `null` rather than throwing — and a
 * partially-recognised tree is repaired toward something usable instead of
 * discarded.
 *
 * Not persisted: which PTY a terminal pane was attached to. PTYs are
 * server-side and keyed by a uuid this tab no longer holds after a reload, so a
 * terminal pane restores as a terminal for its project and spawns a fresh
 * shell. Pretending otherwise would restore a pane wired to nothing.
 */

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
  // Zen is deliberately not restored. `rail` and `paneHeaders` are standing
  // preferences; Zen is where you happened to be when the tab closed, and
  // coming back to a chromeless shell you did not ask for reads as a bug.
  return { rail: flag("rail"), paneHeaders: flag("paneHeaders"), zen: false };
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
    return { type: "leaf", id: asPaneId(value.id), widget: parseWidget(value.widget) };
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

function parseWidget(value: unknown): WidgetState | null {
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
      return { kind: "files", projectPath, open: parseFileOpen(value.open) };
    case "terminal":
      return {
        kind: "terminal",
        projectPath,
        ptyIds: Array.isArray(value.ptyIds)
          ? value.ptyIds.filter((id): id is string => typeof id === "string")
          : [],
      };
    case "git":
      return { kind: "git", projectPath };
  }
}
