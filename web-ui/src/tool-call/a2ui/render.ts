import { esc } from "./types";
import type { A2UIBlock } from "./types";
import { CORE_RENDERERS } from "./blocks";
import { EXT_RENDERERS } from "./blocksExt";
import { MEDIA_RENDERERS } from "./blocksMedia";
import { renderChart } from "./charts";

/** Convert an array of A2UI blocks to an HTML string. */
export function blocksToHtml(blocks: A2UIBlock[]): string {
  let html = "";
  for (const block of blocks) {
    if (!block || typeof block !== "object") continue;
    const { type, data } = block;
    if (!type || !data) continue;

    const d = data as Record<string, unknown>;

    // Core
    const coreFn = CORE_RENDERERS[type];
    if (coreFn) { html += coreFn(d); continue; }

    // Extended (button, form, steps, divider, code, metric, grid, flex, tabs, etc.)
    const extFn = EXT_RENDERERS[type];
    if (extFn) { html += extFn(d); continue; }

    // Media & interface (image, pdf, link, accordion, mermaid, diff, etc.)
    const mediaFn = MEDIA_RENDERERS[type];
    if (mediaFn) { html += mediaFn(d); continue; }

    // Chart
    if (type === "chart") { html += renderChart(d); continue; }

    // Unknown
    html += `<div class="a2ui-unknown">Unknown block: ${esc(type)}</div>`;
  }
  return html;
}
