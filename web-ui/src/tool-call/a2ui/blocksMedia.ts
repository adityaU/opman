import { esc, sf, sfOr, md, avatarHue } from "./types";
import { svgIcon } from "./blocks";
import { blocksToHtml } from "./render";

// ── Image ─────────────────────────────────────────────────

function renderImage(data: Record<string, unknown>): string {
  const url = sfOr(data, "url", "src");
  if (!url) return "";
  const alt = sf(data, "alt");
  const caption = sf(data, "caption");
  const w = data.width ? ` width="${esc(String(data.width))}"` : "";
  const h2 = data.height ? ` height="${esc(String(data.height))}"` : "";
  let h = `<figure class="a2ui-image"><img class="a2ui-image-el" src="${esc(url)}" alt="${esc(alt)}"${w}${h2} loading="lazy">`;
  if (caption) h += `<figcaption class="a2ui-image-caption">${esc(caption)}</figcaption>`;
  return h + "</figure>";
}

// ── PDF ───────────────────────────────────────────────────

function renderPdf(data: Record<string, unknown>): string {
  const url = sfOr(data, "url", "src");
  if (!url) return "";
  const title = sf(data, "title");
  const height = sf(data, "height") || "400px";
  let h = '<div class="a2ui-pdf">';
  if (title) h += `<div class="a2ui-pdf-title">${esc(title)}</div>`;
  h += `<iframe class="a2ui-pdf-frame" src="${esc(url)}" style="height:${esc(height)}" loading="lazy"></iframe>`;
  h += `<a class="a2ui-pdf-fallback" href="${esc(url)}" target="_blank" rel="noopener">${svgIcon("external-link", 12)} Open PDF</a>`;
  return h + "</div>";
}

// ── Link ──────────────────────────────────────────────────

function renderLink(data: Record<string, unknown>): string {
  const url = sfOr(data, "url", "href");
  const label = sfOr(data, "label", "text");
  const desc = sf(data, "description");
  return `<a class="a2ui-link" href="${esc(url)}" target="_blank" rel="noopener"><div><span class="a2ui-link-label">${esc(label || url)}</span>${desc ? `<div class="a2ui-link-desc">${esc(desc)}</div>` : ""}</div><span class="a2ui-link-icon">${svgIcon("external-link", 14)}</span></a>`;
}

// ── Accordion ─────────────────────────────────────────────

function renderAccordion(data: Record<string, unknown>): string {
  const title = sfOr(data, "title", "label");
  const open = data.open ? " open" : "";
  const content = sfOr(data, "content", "text");
  const children = data.blocks as unknown[] | undefined;
  let body = "";
  if (content) body = md(content);
  if (children?.length) body += blocksToHtml(children as any);
  return `<details class="a2ui-accordion"${open}><summary class="a2ui-accordion-summary">${esc(title)}</summary><div class="a2ui-accordion-body">${body}</div></details>`;
}

// ── Mermaid ───────────────────────────────────────────────

function renderMermaid(data: Record<string, unknown>): string {
  const code = sfOr(data, "content", "text", "code");
  const title = sf(data, "title");
  let h = '<div class="a2ui-mermaid">';
  h += '<div class="a2ui-mermaid-toolbar">';
  h += '<button class="a2ui-mermaid-zoom-btn" data-a2ui-mermaid-zoom="in" title="Zoom in">+</button>';
  h += '<button class="a2ui-mermaid-zoom-btn" data-a2ui-mermaid-zoom="out" title="Zoom out">−</button>';
  h += '<button class="a2ui-mermaid-zoom-btn" data-a2ui-mermaid-zoom="reset" title="Reset">⟲</button>';
  h += '</div>';
  if (title) h += `<div class="a2ui-mermaid-title">${esc(title)}</div>`;
  h += `<div class="a2ui-mermaid-viewport"><pre class="mermaid">${esc(code)}</pre></div>`;
  h += "</div>";
  return h;
}

// ── Diff ──────────────────────────────────────────────────

function renderDiff(data: Record<string, unknown>): string {
  const text = sfOr(data, "content", "text");
  const title = sf(data, "title");
  if (!text) return "";
  let h = '<div class="a2ui-diff">';
  if (title) h += `<div class="a2ui-diff-title">${esc(title)}</div>`;
  h += '<pre class="a2ui-diff-pre">';
  const lines = text.split("\n");
  let ln = 0;
  for (const line of lines) {
    let cls = "a2ui-diff-ctx";
    if (line.startsWith("+")) cls = "a2ui-diff-add";
    else if (line.startsWith("-")) cls = "a2ui-diff-del";
    else if (line.startsWith("@@")) cls = "a2ui-diff-hunk";
    ln++;
    h += `<span class="${cls}"><span class="a2ui-diff-ln">${ln}</span>${esc(line)}\n</span>`;
  }
  h += "</pre></div>";
  return h;
}

// ── Timeline ──────────────────────────────────────────────

function renderTimeline(data: Record<string, unknown>): string {
  const items = (data.items ?? data.entries) as Array<{
    label?: string; title?: string; description?: string; body?: string;
    date?: string; time?: string; status?: string; icon?: string;
  }> | undefined;
  if (!items?.length) return "";
  let h = '<div class="a2ui-timeline">';
  for (const item of items) {
    const st = item.status || "";
    const cls = st ? `a2ui-tl-entry a2ui-tl-${st}` : "a2ui-tl-entry";
    h += `<div class="${cls}"><div class="a2ui-tl-dot">`;
    if (st === "done" || st === "completed") h += svgIcon("check-circle", 12);
    else if (st === "error") h += svgIcon("x-circle", 12);
    else h += '<div class="a2ui-tl-dot-empty"></div>';
    h += "</div><div class=\"a2ui-tl-content\">";
    const date = item.date ?? item.time ?? "";
    if (date) h += `<span class="a2ui-tl-date">${esc(date)}</span>`;
    const label = item.label ?? item.title ?? "";
    if (label) h += `<span class="a2ui-tl-label">${esc(label)}</span>`;
    const desc = item.description ?? item.body ?? "";
    if (desc) h += `<span class="a2ui-tl-desc">${esc(desc)}</span>`;
    h += "</div></div>";
  }
  h += "</div>";
  return h;
}

// ── Terminal ──────────────────────────────────────────────

function ansiToHtml(text: string): string {
  let h = esc(text);
  // Basic ANSI code replacement
  const map: Record<string, string> = {
    "1": "a2ui-ansi-bold", "2": "a2ui-ansi-dim",
    "31": "a2ui-ansi-red", "32": "a2ui-ansi-green", "33": "a2ui-ansi-yellow",
    "34": "a2ui-ansi-blue", "35": "a2ui-ansi-magenta", "36": "a2ui-ansi-cyan",
  };
  // Replace ESC[Nm sequences (already escaped, so &amp; etc won't appear mid-sequence)
  h = h.replace(/\x1b\[(\d+)m/g, (_, code) => {
    if (code === "0") return "</span>";
    const cls = map[code];
    return cls ? `<span class="${cls}">` : "";
  });
  return h;
}

function renderTerminal(data: Record<string, unknown>): string {
  const text = sfOr(data, "content", "text");
  const title = sf(data, "title") || "Terminal";
  const prompt = sf(data, "prompt");
  let h = '<div class="a2ui-terminal"><div class="a2ui-term-titlebar"><div class="a2ui-term-dots">';
  h += '<span class="a2ui-term-dot a2ui-term-dot-r"></span>';
  h += '<span class="a2ui-term-dot a2ui-term-dot-y"></span>';
  h += '<span class="a2ui-term-dot a2ui-term-dot-g"></span>';
  h += `</div><span class="a2ui-term-title">${esc(title)}</span></div>`;
  h += '<pre class="a2ui-term-body">';
  if (prompt) h += `<span class="a2ui-term-prompt">${esc(prompt)} </span>`;
  h += ansiToHtml(text);
  h += "</pre></div>";
  return h;
}

// ── File Tree ─────────────────────────────────────────────

const FILE_ICONS: Record<string, string> = {
  rs: "🦀", js: "🟨", ts: "🔷", py: "🐍", css: "🎨", html: "🌐",
  json: "📋", toml: "⚙️", yaml: "⚙️", yml: "⚙️", md: "📝", lock: "🔒",
};

function fileIcon(name: string): string {
  const ext = name.includes(".") ? name.split(".").pop()! : "";
  return FILE_ICONS[ext] ?? "📄";
}

function renderTreeNodes(items: Array<{ name: string; type?: string; status?: string; items?: unknown[]; children?: unknown[] }>, depth: number): string {
  if (depth > 10 || !items?.length) return "";
  let h = '<ul class="a2ui-ftree-list">';
  for (const node of items) {
    const isDir = node.type === "dir" || node.type === "directory";
    const stCls = node.status ? ` a2ui-ftree-${node.status}` : "";
    h += `<li class="a2ui-ftree-node${isDir ? " a2ui-ftree-dir" : ""}${stCls}">`;
    const kids = node.items ?? node.children;
    if (isDir && kids) {
      h += `<details open><summary><span class="a2ui-ftree-name"><span class="a2ui-ftree-icon">📁</span>${esc(node.name)}</span></summary>`;
      h += renderTreeNodes(kids as any, depth + 1);
      h += "</details>";
    } else {
      h += `<span class="a2ui-ftree-name"><span class="a2ui-ftree-icon">${fileIcon(node.name)}</span>${esc(node.name)}</span>`;
    }
    h += "</li>";
  }
  h += "</ul>";
  return h;
}

function renderFileTree(data: Record<string, unknown>): string {
  const items = (data.items ?? data.tree) as Array<any> | undefined;
  const title = sf(data, "title");
  if (!items?.length) return "";
  let h = '<div class="a2ui-ftree">';
  if (title) h += `<div class="a2ui-ftree-title">${esc(title)}</div>`;
  h += renderTreeNodes(items, 0);
  h += "</div>";
  return h;
}

// ── Avatar ────────────────────────────────────────────────

function renderSingleAvatar(a: { src?: string; url?: string; name?: string; initials?: string }, size: string): string {
  const s = a.src ?? a.url ?? "";
  if (s) return `<img class="a2ui-avatar" src="${esc(s)}" alt="${esc(a.name ?? "")}">`;
  const ini = a.initials ?? (a.name ? a.name.split(" ").map(w => w[0]).join("").slice(0, 2).toUpperCase() : "?");
  const hue = avatarHue(a.name ?? ini);
  return `<span class="a2ui-avatar a2ui-avatar-initials" style="--avatar-hue:${hue}">${esc(ini)}</span>`;
}

function renderAvatar(data: Record<string, unknown>): string {
  const size = sf(data, "size") || "md";
  const avatars = data.avatars as Array<any> | undefined;
  if (avatars?.length) {
    let h = `<div class="a2ui-avatar-group a2ui-avatar-${size}">`;
    for (const a of avatars) h += renderSingleAvatar(a, size);
    return h + "</div>";
  }
  const name = sf(data, "name");
  let h = `<div class="a2ui-avatar-single a2ui-avatar-${size}">`;
  h += renderSingleAvatar(data as any, size);
  if (name) h += `<span class="a2ui-avatar-name">${esc(name)}</span>`;
  return h + "</div>";
}

// ── Tag Group ─────────────────────────────────────────────

function renderTagGroup(data: Record<string, unknown>): string {
  const tags = data.tags as Array<{ label: string; variant?: string; selected?: boolean }> | undefined;
  if (!tags?.length) return "";
  const cbId = sf(data, "callback_id");
  let h = '<div class="a2ui-tag-group">';
  for (const t of tags) {
    const cls = ["a2ui-tag", t.variant ? `a2ui-tag-${t.variant}` : "", t.selected ? "a2ui-tag-selected" : ""].filter(Boolean).join(" ");
    if (cbId) {
      h += `<button class="${cls}" data-a2ui-callback="${esc(cbId)}" data-a2ui-tag-value="${esc(t.label)}">${esc(t.label)}</button>`;
    } else {
      h += `<span class="${cls}">${esc(t.label)}</span>`;
    }
  }
  return h + "</div>";
}

// ── Toggle ────────────────────────────────────────────────

let _toggleId = 0;

function renderToggle(data: Record<string, unknown>): string {
  const label = sf(data, "label");
  const checked = Boolean(data.checked ?? data.value);
  const cbId = sf(data, "callback_id");
  const desc = sf(data, "description");
  const id = `a2tg${_toggleId++}`;
  let h = '<div class="a2ui-toggle">';
  h += `<span class="a2ui-toggle-label">${esc(label)}</span>`;
  h += '<label class="a2ui-toggle-switch">';
  h += `<input type="checkbox" class="a2ui-toggle-input" id="${id}"${checked ? " checked" : ""}${cbId ? ` data-a2ui-callback="${esc(cbId)}"` : ""}>`;
  h += '<span class="a2ui-toggle-track"></span>';
  h += '</label>';
  if (desc) h += `<span class="a2ui-toggle-desc">${esc(desc)}</span>`;
  h += "</div>";
  return h;
}

// ── Video / Audio ─────────────────────────────────────────

function renderVideo(data: Record<string, unknown>): string {
  const url = sfOr(data, "url", "src");
  if (!url) return "";
  const poster = sf(data, "poster");
  const caption = sfOr(data, "title", "caption");
  const w = data.width ? ` width="${esc(String(data.width))}"` : "";
  const h2 = data.height ? ` height="${esc(String(data.height))}"` : "";
  let h = '<figure class="a2ui-video">';
  h += `<video class="a2ui-video-el" controls${poster ? ` poster="${esc(poster)}"` : ""}${w}${h2}><source src="${esc(url)}"></video>`;
  if (caption) h += `<figcaption class="a2ui-video-caption">${esc(caption)}</figcaption>`;
  return h + "</figure>";
}

function renderAudio(data: Record<string, unknown>): string {
  const url = sfOr(data, "url", "src");
  if (!url) return "";
  const title = sfOr(data, "title", "label");
  let h = '<div class="a2ui-audio">';
  if (title) h += `<span class="a2ui-audio-title">${esc(title)}</span>`;
  h += `<audio class="a2ui-audio-el" controls><source src="${esc(url)}"></audio>`;
  return h + "</div>";
}

// ── Separator ─────────────────────────────────────────────

function renderSeparator(data: Record<string, unknown>): string {
  const icon = sfOr(data, "icon", "emoji");
  const label = sf(data, "label");
  const style = sf(data, "style") || "solid";
  const cls = `a2ui-separator a2ui-sep-${style}`;
  let h = `<div class="${cls}"><hr class="a2ui-sep-line">`;
  if (icon) h += `<span class="a2ui-sep-icon">${esc(icon)}</span>`;
  if (label) h += `<span class="a2ui-sep-label">${esc(label)}</span>`;
  if (icon || label) h += '<hr class="a2ui-sep-line">';
  return h + "</div>";
}

export const MEDIA_RENDERERS: Record<string, (data: Record<string, unknown>) => string> = {
  image: renderImage,
  pdf: renderPdf,
  link: renderLink,
  accordion: renderAccordion,
  mermaid: renderMermaid,
  diff: renderDiff,
  timeline: renderTimeline,
  terminal: renderTerminal,
  "file-tree": renderFileTree,
  file_tree: renderFileTree,
  avatar: renderAvatar,
  "tag-group": renderTagGroup,
  tag_group: renderTagGroup,
  toggle: renderToggle,
  video: renderVideo,
  audio: renderAudio,
  separator: renderSeparator,
};
