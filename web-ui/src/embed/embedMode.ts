/**
 * Whether this page load is an embed: one widget, inside another instance's
 * pane. Decided once from the URL, like `BASE` — an instance never changes
 * into or out of being an embed.
 */

import { SERVER_ID } from "../api/base";
import type { WidgetState } from "../workspace/types";
import { parseEmbedSearch } from "./embedUrl";
import type { EmbedToParent } from "./messages";
import { emulateWideViewport } from "./wideViewport";

export const EMBED_WIDGET: WidgetState | null =
  typeof window === "undefined" ? null : parseEmbedSearch(window.location.search);

export const IS_EMBED = EMBED_WIDGET !== null;

/** Distributive omit, so each arm keeps its own fields. */
type WithoutServer<T> = T extends unknown ? Omit<T, "server"> : never;

/** Tell the parent something. A no-op outside a frame — there is nobody to tell. */
export function postToParent(message: WithoutServer<EmbedToParent>): void {
  if (!IS_EMBED || window.parent === window) return;
  window.parent.postMessage({ ...message, server: SERVER_ID }, location.origin);
}

/**
 * Boot-time setup, before the first render: mark `<html>` so the stylesheets
 * can drop every bit of shell chrome, and make the layout behave as on a
 * desktop however narrow the pane is.
 */
export function initEmbedMode(): void {
  if (!IS_EMBED) return;
  document.documentElement.setAttribute("data-opman-embed", "");
  emulateWideViewport();
}
