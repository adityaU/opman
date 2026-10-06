/**
 * Parsing one widget out of untrusted JSON.
 *
 * Split from `persistence.ts` because a widget arrives from more places than
 * the saved layout: an embedded pane is handed one in its URL, and an embed
 * forwards one to its parent with `postMessage`. All three must agree on what a
 * well-formed widget is, so they share this one total parser.
 */

import { browserIdForProject } from "../api/browser";
import {
  WIDGET_KINDS,
  type FileOpenRequest,
  type PaneEngine,
  type PaneId,
  type WidgetKind,
  type WidgetState,
} from "./types";

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
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
 * Which server a widget belongs to. Absent, empty or malformed all read as
 * "this instance's own", which is also what every layout saved before servers
 * existed means.
 */
function parseServer(value: unknown): string | undefined {
  return typeof value === "string" && value ? value : undefined;
}

/**
 * A widget from untrusted JSON — a saved layout, an `?embed=` parameter, a
 * message from another frame — or null when it is not one.
 *
 * `paneId` scopes a files pane's language servers when the value carries no
 * session of its own.
 */
export function parseWidget(value: unknown, paneId: PaneId): WidgetState | null {
  const widget = parseArm(value, paneId);
  if (!widget || !isRecord(value)) return widget;
  const server = parseServer(value.server);
  return server ? { ...widget, server } : widget;
}

function parseArm(value: unknown, paneId: PaneId): WidgetState | null {
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
