/**
 * The embed URL: how one opman instance shows a single widget from another
 * server inside a pane.
 *
 * Contract (docs/multi-server-device-browser.md):
 * `${serverBase(server)}/?embed=${encodeURIComponent(JSON.stringify(widget))}`,
 * the widget without `server` — inside the embed it *is* the own server.
 */

import { serverBase } from "../api/base";
import { parseWidget } from "../workspace/parseWidget";
import { asPaneId, type WidgetState } from "../workspace/types";

/** The pane id an embed's lone pane is parsed against. */
export const EMBED_PANE = asPaneId("embed");

export function stripServer(widget: WidgetState): WidgetState {
  if (widget.server === undefined) return widget;
  const { server: _server, ...rest } = widget;
  return rest;
}

/** Identity of a widget for "is this the one already showing", ignoring `server`. */
export function widgetKey(widget: WidgetState): string {
  return JSON.stringify(stripServer(widget));
}

export function embedUrl(server: string, widget: WidgetState): string {
  return `${serverBase(server)}/?embed=${encodeURIComponent(widgetKey(widget))}`;
}

/** The widget a page was asked to embed, or null for a normal page load. */
export function parseEmbedSearch(search: string): WidgetState | null {
  const raw = new URLSearchParams(search).get("embed");
  if (!raw) return null;
  try {
    const widget = parseWidget(JSON.parse(raw), EMBED_PANE);
    return widget ? stripServer(widget) : null;
  } catch {
    return null;
  }
}
