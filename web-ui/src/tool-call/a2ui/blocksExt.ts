import { esc, sf, sfOr, md } from "./types";
import { svgIcon, levelIcon } from "./blocks";
import { blocksToHtml } from "./render";

let _idCounter = 0;
function nextId(prefix: string): string { return `${prefix}${_idCounter++}`; }

// ── Button ────────────────────────────────────────────────

function renderButton(data: Record<string, unknown>): string {
  const label = sf(data, "label");
  const cbId = sf(data, "callback_id");
  const variant = sf(data, "variant");
  const cls = variant ? `a2ui-btn a2ui-btn-${esc(variant)}` : "a2ui-btn";
  return cbId
    ? `<button class="${cls}" data-a2ui-callback="${esc(cbId)}">${esc(label)}</button>`
    : `<button class="${cls}">${esc(label)}</button>`;
}

// ── Form ──────────────────────────────────────────────────

function renderForm(data: Record<string, unknown>): string {
  const cbId = sf(data, "callback_id");
  const submitLabel = sf(data, "submit_label") || "Submit";
  const fields = data.fields as Array<{
    name: string; label?: string; type?: string;
    placeholder?: string; default?: string;
  }> | undefined;
  let h = `<form class="a2ui-form" data-a2ui-form-callback="${esc(cbId)}">`;
  if (fields) {
    for (const f of fields) {
      h += '<div class="a2ui-form-field">';
      if (f.label) h += `<label class="a2ui-form-label">${esc(f.label)}</label>`;
      if (f.type === "textarea") {
        h += `<textarea class="a2ui-form-input" name="${esc(f.name)}" placeholder="${esc(f.placeholder ?? "")}">${esc(f.default ?? "")}</textarea>`;
      } else {
        h += `<input class="a2ui-form-input" type="${esc(f.type || "text")}" name="${esc(f.name)}" placeholder="${esc(f.placeholder ?? "")}" value="${esc(f.default ?? "")}">`;
      }
      h += "</div>";
    }
  }
  h += `<button type="submit" class="a2ui-btn a2ui-btn-primary">${esc(submitLabel)}</button></form>`;
  return h;
}

// ── Steps ─────────────────────────────────────────────────

function renderSteps(data: Record<string, unknown>): string {
  const title = sf(data, "title");
  const items = (data.items ?? data.steps) as Array<{ label: string; status?: string }> | undefined;
  if (!items?.length) return "";
  let h = '<div class="a2ui-steps">';
  if (title) h += `<div class="a2ui-steps-title">${esc(title)}</div>`;
  h += '<ol class="a2ui-steps-list">';
  items.forEach((item, i) => {
    const st = item.status || "pending";
    const cls = `a2ui-step a2ui-step-${st}`;
    let icon = "";
    if (st === "done" || st === "completed") icon = svgIcon("check-circle", 14);
    else if (st === "error") icon = svgIcon("x-circle", 14);
    else if (st === "active" || st === "in_progress") icon = `<span class="a2ui-step-num">${i + 1}</span>`;
    else icon = `<span class="a2ui-step-num">${i + 1}</span>`;
    h += `<li class="${cls}"><span class="a2ui-step-icon">${icon}</span><span class="a2ui-step-label">${esc(item.label)}</span></li>`;
  });
  h += "</ol></div>";
  return h;
}

// ── Divider ───────────────────────────────────────────────

function renderDivider(data: Record<string, unknown>): string {
  const label = sf(data, "label");
  if (!label) return '<div class="a2ui-divider"><hr class="a2ui-divider-line"></div>';
  return `<div class="a2ui-divider"><hr class="a2ui-divider-line"><span class="a2ui-divider-label">${esc(label)}</span><hr class="a2ui-divider-line"></div>`;
}

// ── Code ──────────────────────────────────────────────────

function renderCode(data: Record<string, unknown>): string {
  const code = sfOr(data, "code", "content");
  const lang = sf(data, "language");
  return `<div class="a2ui-code"><pre class="tool-call-pre"><code${lang ? ` class="language-${esc(lang)}"` : ""}>${esc(code)}</code></pre></div>`;
}

// ── Metric ────────────────────────────────────────────────

function renderMetric(data: Record<string, unknown>): string {
  const label = sf(data, "label");
  const value = String(data.value ?? "");
  const trend = sf(data, "trend");
  const desc = sf(data, "description");
  let trendHtml = "";
  if (trend === "up") trendHtml = '<span class="a2ui-metric-trend a2ui-metric-trend-up">↑</span>';
  else if (trend === "down") trendHtml = '<span class="a2ui-metric-trend a2ui-metric-trend-down">↓</span>';
  else if (trend === "flat") trendHtml = '<span class="a2ui-metric-trend a2ui-metric-trend-flat">→</span>';
  return `<div class="a2ui-metric"><span class="a2ui-metric-label">${esc(label)}</span><span class="a2ui-metric-value">${esc(value)}</span>${trendHtml}${desc ? `<span class="a2ui-metric-desc">${esc(desc)}</span>` : ""}</div>`;
}

// ── Grid / Flex ───────────────────────────────────────────

function renderGrid(data: Record<string, unknown>): string {
  const cols = Math.min(12, Math.max(1, Number(data.columns ?? 2)));
  const gap = sf(data, "gap") || "var(--space-2)";
  const minW = sf(data, "min_col_width");
  const children = data.blocks as Array<Record<string, unknown>> | undefined;
  const tpl = minW ? `repeat(auto-fit, minmax(${minW}, 1fr))` : `repeat(${cols}, 1fr)`;
  let h = `<div class="a2ui-grid" style="grid-template-columns:${tpl};gap:${gap}">`;
  if (children) {
    for (const child of children) {
      h += `<div class="a2ui-grid-cell">${blocksToHtml([child as any])}</div>`;
    }
  }
  h += "</div>";
  return h;
}

function renderFlex(data: Record<string, unknown>): string {
  const dir = sf(data, "direction") || "row";
  const gap = sf(data, "gap") || "var(--space-2)";
  const align = sf(data, "align");
  const justify = sf(data, "justify");
  const wrap = data.wrap ? "flex-wrap:wrap;" : "";
  let style = `flex-direction:${dir};gap:${gap};${wrap}`;
  if (align) style += `align-items:${align};`;
  if (justify) style += `justify-content:${justify};`;
  const children = data.blocks as Array<Record<string, unknown>> | undefined;
  let h = `<div class="a2ui-flex" style="${style}">`;
  if (children) {
    for (const child of children) {
      h += `<div class="a2ui-flex-item">${blocksToHtml([child as any])}</div>`;
    }
  }
  h += "</div>";
  return h;
}

// ── Tabs ──────────────────────────────────────────────────

function renderTabs(data: Record<string, unknown>): string {
  const tabs = data.tabs as Array<{ label: string; blocks?: unknown[] }> | undefined;
  if (!tabs?.length) return "";
  const active = Number(data.active ?? 0);
  const groupName = nextId("a2t");
  let h = '<div class="a2ui-tabs"><div class="a2ui-tabs-header">';
  tabs.forEach((tab, i) => {
    const id = `${groupName}_${i}`;
    h += `<input type="radio" class="a2ui-tabs-radio" name="${groupName}" id="${id}"${i === active ? " checked" : ""}>`;
    h += `<label class="a2ui-tabs-label" for="${id}">${esc(tab.label)}</label>`;
  });
  h += '</div><div class="a2ui-tabs-panels">';
  for (const tab of tabs) {
    h += `<div class="a2ui-tab-panel">${tab.blocks ? blocksToHtml(tab.blocks as any) : ""}</div>`;
  }
  h += "</div></div>";
  return h;
}

// ── Callout ───────────────────────────────────────────────

function renderCallout(data: Record<string, unknown>): string {
  const variant = sf(data, "variant") || "info";
  const title = sf(data, "title");
  const body = sfOr(data, "body", "content");
  const children = data.blocks as unknown[] | undefined;
  let h = `<div class="a2ui-callout a2ui-callout-${esc(variant)}">`;
  if (title) h += `<div class="a2ui-callout-header">${levelIcon(variant === "tip" ? "success" : variant === "danger" ? "error" : variant, 14)}<span>${esc(title)}</span></div>`;
  if (body) h += `<div class="a2ui-callout-body">${md(body)}</div>`;
  if (children?.length) h += `<div class="a2ui-callout-body">${blocksToHtml(children as any)}</div>`;
  h += "</div>";
  return h;
}

// ── Badge ─────────────────────────────────────────────────

function renderBadge(data: Record<string, unknown>): string {
  const badges = data.badges as Array<{ label: string; variant?: string }> | undefined;
  if (badges?.length) {
    let h = '<span class="a2ui-badge-group">';
    for (const b of badges) h += `<span class="a2ui-badge a2ui-badge-${b.variant || "neutral"}">${esc(b.label)}</span>`;
    return h + "</span>";
  }
  const label = sf(data, "label");
  const variant = sf(data, "variant") || "neutral";
  return `<span class="a2ui-badge a2ui-badge-${esc(variant)}">${esc(label)}</span>`;
}

// ── Blockquote ────────────────────────────────────────────

function renderBlockquote(data: Record<string, unknown>): string {
  const text = sfOr(data, "content", "text");
  const attr = sfOr(data, "attribution", "author", "cite");
  return `<blockquote class="a2ui-blockquote">${md(text)}${attr ? `<div class="a2ui-blockquote-attr">— ${esc(attr)}</div>` : ""}</blockquote>`;
}

// ── List ──────────────────────────────────────────────────

function renderListItems(items: Array<{ text: string; icon?: string; description?: string; items?: unknown[] }>): string {
  let h = "";
  for (const item of items) {
    h += '<li class="a2ui-list-item">';
    if (item.icon) h += `<span class="a2ui-list-icon">${esc(item.icon)}</span>`;
    h += `<span class="a2ui-list-text">${esc(item.text)}</span>`;
    if (item.description) h += `<span class="a2ui-list-desc">${esc(item.description)}</span>`;
    if (item.items?.length) h += `<ul class="a2ui-list a2ui-list-nested">${renderListItems(item.items as any)}</ul>`;
    h += "</li>";
  }
  return h;
}

function renderList(data: Record<string, unknown>): string {
  const items = data.items as Array<{ text: string; icon?: string; description?: string; items?: unknown[] }> | undefined;
  if (!items?.length) return "";
  const tag = data.ordered ? "ol" : "ul";
  return `<${tag} class="a2ui-list">${renderListItems(items)}</${tag}>`;
}

// ── Stat Group ────────────────────────────────────────────

function renderStatGroup(data: Record<string, unknown>): string {
  const stats = (data.stats as Array<{ label: string; value: string; trend?: string; description?: string }>) ?? [];
  if (!stats.length) return "";
  const cols = Math.min(6, stats.length);
  let h = `<div class="a2ui-stat-group" style="grid-template-columns:repeat(${cols},1fr)">`;
  for (const s of stats) {
    let trend = "";
    if (s.trend === "up") trend = '<span class="a2ui-trend-up">↑</span>';
    else if (s.trend === "down") trend = '<span class="a2ui-trend-down">↓</span>';
    else if (s.trend) trend = '<span class="a2ui-trend-flat">→</span>';
    h += `<div class="a2ui-stat"><span class="a2ui-stat-label">${esc(s.label)}</span><div class="a2ui-stat-row"><span class="a2ui-stat-value">${esc(String(s.value))}</span>${trend}</div>${s.description ? `<span class="a2ui-stat-desc">${esc(s.description)}</span>` : ""}</div>`;
  }
  h += "</div>";
  return h;
}

export const EXT_RENDERERS: Record<string, (data: Record<string, unknown>) => string> = {
  button: renderButton,
  form: renderForm,
  steps: renderSteps,
  divider: renderDivider,
  code: renderCode,
  metric: renderMetric,
  grid: renderGrid,
  flex: renderFlex,
  tabs: renderTabs,
  callout: renderCallout,
  badge: renderBadge,
  blockquote: renderBlockquote,
  list: renderList,
  "stat-group": renderStatGroup,
  stat_group: renderStatGroup,
};
