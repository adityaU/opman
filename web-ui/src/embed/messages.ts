/**
 * The postMessage protocol between a pane's iframe (the embed) and the
 * instance showing it (the parent). Same origin always: every server's
 * instance is served by home, so anything from another origin is not ours.
 *
 * Embed → parent:
 * - `opman:open-widget` — show this widget somewhere (MCP open, a file link);
 *   the parent places it with its normal planners, tagged with `server`.
 * - `opman:widget-changed` — the embed's own widget moved on (a session was
 *   created, a file opened); the parent records it so a reload comes back to it.
 * - `opman:embed-ready` — the embed has mounted and can take `opman:set-widget`.
 * - `opman:run-command` — a `workspace.*` chord pressed inside the iframe; the
 *   parent owns the panes it acts on.
 *
 * Parent → embed:
 * - `opman:set-widget` — the parent pointed the pane elsewhere (history step,
 *   a forwarded open landing on it).
 */

import { parseWidget, isRecord } from "../workspace/parseWidget";
import type { WidgetState } from "../workspace/types";
import { EMBED_PANE, stripServer } from "./embedUrl";

export type EmbedToParent =
  | { readonly type: "opman:open-widget"; readonly server: string; readonly widget: WidgetState }
  | { readonly type: "opman:widget-changed"; readonly server: string; readonly widget: WidgetState }
  | { readonly type: "opman:embed-ready"; readonly server: string }
  | { readonly type: "opman:run-command"; readonly server: string; readonly command: string };

export interface ParentToEmbed {
  readonly type: "opman:set-widget";
  readonly widget: WidgetState;
}

/**
 * A widget from a live message. Unlike a saved layout, a browser's `reveal`
 * is kept: here it is a fresh request to navigate, not a stale counter.
 */
export function parseLiveWidget(value: unknown): WidgetState | null {
  const widget = parseWidget(value, EMBED_PANE);
  if (!widget) return null;
  if (widget.kind === "browser" && isRecord(value) && typeof value.reveal === "number") {
    return { ...widget, reveal: value.reveal };
  }
  return widget;
}

export function isSameOrigin(event: { readonly origin: string }, origin: string = location.origin): boolean {
  return event.origin === origin;
}

export function readFromEmbed(data: unknown): EmbedToParent | null {
  if (!isRecord(data) || typeof data.server !== "string" || !data.server) return null;
  const server = data.server;
  switch (data.type) {
    case "opman:embed-ready":
      return { type: "opman:embed-ready", server };
    case "opman:run-command":
      return typeof data.command === "string" && data.command.startsWith("workspace.")
        ? { type: "opman:run-command", server, command: data.command }
        : null;
    case "opman:open-widget":
    case "opman:widget-changed": {
      const widget = parseLiveWidget(data.widget);
      return widget ? { type: data.type, server, widget: stripServer(widget) } : null;
    }
    default:
      return null;
  }
}

export function readFromParent(data: unknown): ParentToEmbed | null {
  if (!isRecord(data) || data.type !== "opman:set-widget") return null;
  const widget = parseLiveWidget(data.widget);
  return widget ? { type: "opman:set-widget", widget: stripServer(widget) } : null;
}
