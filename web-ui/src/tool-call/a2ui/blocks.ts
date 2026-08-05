import { esc, sf, sfOr, md, mdInline } from "./types";
// md()      — block-level markdown (headings, lists, code fences, paragraphs)
// mdInline()— inline markdown (bold, italic, code, links — no wrapping <p>)
// esc()     — plain HTML-escape for identifiers, labels, attribute values
import type { A2UIBlock } from "./types";

export function svgIcon(name: string, size = 14): string {
  const s = `width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"`;
  switch (name) {
    case "check-circle": return `<svg ${s}><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>`;
    case "alert-triangle": return `<svg ${s}><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>`;
    case "x-circle": return `<svg ${s}><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>`;
    case "info": return `<svg ${s}><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>`;
    case "external-link": return `<svg ${s}><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>`;
    default: return "";
  }
}

export function levelIcon(level: string, size = 14): string {
  switch (level) {
    case "success": return svgIcon("check-circle", size);
    case "warning": return svgIcon("alert-triangle", size);
    case "error":   return svgIcon("x-circle", size);
    default:        return svgIcon("info", size);
  }
}

// ── Core blocks ────────────────────────────────────────────────

function renderCard(data: Record<string, unknown>): string {
  const title = sf(data, "title");
  const icon = sf(data, "icon");
  const body = sfOr(data, "body", "content");
  let h = '<div class="a2ui-card">';
  if (title) {
    h += '<div class="a2ui-card-header">';
    if (icon) h += `<span class="a2ui-card-icon">${esc(icon)}</span>`;
    h += `<span class="a2ui-card-title">${esc(title)}</span></div>`;
  }
  if (body) h += `<div class="a2ui-card-body">${md(body)}</div>`;
  h += "</div>";
  return h;
}

function renderTable(data: Record<string, unknown>): string {
  const headers = data.headers as string[] | undefined;
  const rows = data.rows as string[][] | undefined;
  let h = '<div class="a2ui-table-wrap"><table class="a2ui-table">';
  if (headers?.length) {
    h += "<thead><tr>";
    for (const hdr of headers) h += `<th>${esc(hdr)}</th>`;
    h += "</tr></thead>";
  }
  if (rows?.length) {
    h += "<tbody>";
    for (const row of rows) {
      h += "<tr>";
      for (const cell of row) h += `<td>${mdInline(String(cell))}</td>`;
      h += "</tr>";
    }
    h += "</tbody>";
  }
  h += "</table></div>";
  return h;
}

function renderKv(data: Record<string, unknown>): string {
  const pairs = data.pairs as Array<{ key: string; value: string }> | undefined;
  if (!pairs?.length) return "";
  let h = '<div class="a2ui-kv">';
  for (const p of pairs) {
    h += `<div class="a2ui-kv-row"><span class="a2ui-kv-key">${esc(p.key)}</span><span class="a2ui-kv-val">${mdInline(p.value)}</span></div>`;
  }
  h += "</div>";
  return h;
}

function renderStatus(data: Record<string, unknown>): string {
  const label = sf(data, "label");
  const level = sf(data, "level") || "info";
  const detail = sfOr(data, "detail", "message");
  return `<div class="a2ui-status a2ui-status-${esc(level)}">${levelIcon(level)}<span class="a2ui-status-label">${esc(label)}</span>${detail ? `<span class="a2ui-status-detail">${mdInline(detail)}</span>` : ""}</div>`;
}

function renderProgress(data: Record<string, unknown>): string {
  const label = sf(data, "label");
  const pct = Math.max(0, Math.min(100, Number(data.percent ?? data.percentage ?? 0)));
  return `<div class="a2ui-progress"><div class="a2ui-progress-header"><span class="a2ui-progress-label">${esc(label)}</span><span class="a2ui-progress-pct">${pct}%</span></div><div class="a2ui-progress-track"><div class="a2ui-progress-fill" style="width:${pct}%"></div></div></div>`;
}

function renderAlert(data: Record<string, unknown>): string {
  // `message` is canonical; tolerate common aliases so malformed model output
  // does not leave an otherwise valid alert visibly blank.
  const msg = sfOr(data, "message", "content", "body", "text");
  const level = sf(data, "level") || "info";
  return `<div class="a2ui-alert a2ui-alert-${esc(level)}">${levelIcon(level, 16)}<div>${md(msg)}</div></div>`;
}

function renderMarkdown(data: Record<string, unknown>): string {
  const content = sfOr(data, "content", "text");
  return `<div class="a2ui-markdown">${md(content)}</div>`;
}

// Map block type -> renderer
const CORE_RENDERERS: Record<string, (data: Record<string, unknown>) => string> = {
  card: renderCard,
  table: renderTable,
  kv: renderKv,
  status: renderStatus,
  progress: renderProgress,
  alert: renderAlert,
  markdown: renderMarkdown,
};

export { CORE_RENDERERS };
