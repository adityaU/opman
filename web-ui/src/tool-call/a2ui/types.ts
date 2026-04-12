// A2UI types used across renderer files.

export interface A2UIBlock {
  type: string;
  data: Record<string, unknown>;
}

export interface ChartDataset {
  label?: string;
  data: number[];
  color?: string;
}

export interface ChartValue {
  label: string;
  value: number;
  color?: string;
}

/** Extract blocks array from tool call input. */
export function extractBlocks(input: unknown): { title?: string; blocks: A2UIBlock[] } {
  if (!input || typeof input !== "object") return { blocks: [] };

  const obj = input as Record<string, unknown>;

  // Direct array
  if (Array.isArray(input)) return { blocks: input as A2UIBlock[] };

  // Object with blocks array
  const blocks = Array.isArray(obj.blocks) ? (obj.blocks as A2UIBlock[]) : [];
  const title = typeof obj.title === "string" ? obj.title : undefined;
  return { title, blocks };
}

/** HTML-escape special characters (safe for undefined/non-string input) */
export function esc(s: unknown): string {
  if (typeof s !== "string") return s == null ? "" : String(s);
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** Safely extract a string field from block data */
export function sf(data: Record<string, unknown>, key: string): string {
  const v = data[key];
  return typeof v === "string" ? v : "";
}

/** Extract first matching string field */
export function sfOr(data: Record<string, unknown>, ...keys: string[]): string {
  for (const k of keys) {
    const v = data[k];
    if (typeof v === "string" && v.length > 0) return v;
  }
  return "";
}

/** Simple markdown-to-HTML: bold, italic, code, links, paragraphs */
export function md(text: unknown): string {
  if (!text || typeof text !== "string") return "";
  let h = esc(text);
  // inline code
  h = h.replace(/`([^`]+)`/g, '<code>$1</code>');
  // bold
  h = h.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  // italic
  h = h.replace(/\*(.+?)\*/g, '<em>$1</em>');
  // links
  h = h.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
  // line breaks -> paragraphs
  const paras = h.split(/\n{2,}/);
  if (paras.length <= 1) return `<p>${h}</p>`;
  return paras.map((p) => `<p>${p.replace(/\n/g, "<br>")}</p>`).join("");
}

/** Inline markdown — strips outer <p> for single-line fields */
export function mdInline(text: unknown): string {
  if (!text || typeof text !== "string") return "";
  const result = md(text);
  return result.replace(/^<p>(.*)<\/p>$/s, "$1");
}

const CHART_COLORS = [
  "#6366f1", "#22d3ee", "#f59e0b", "#ef4444", "#10b981",
  "#8b5cf6", "#f97316", "#ec4899", "#14b8a6", "#64748b",
];

export function chartColor(i: number): string {
  return CHART_COLORS[i % CHART_COLORS.length];
}

/** Compute hue from a name string for avatar */
export function avatarHue(name: string): number {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = ((hash * 31) + name.charCodeAt(i)) | 0;
  }
  return Math.abs(hash) % 360;
}

/** Format large numbers with K/M suffix */
export function fmtNum(n: number): string {
  if (Math.abs(n) >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (Math.abs(n) >= 10_000) return `${(n / 1_000).toFixed(0)}K`;
  if (Math.abs(n) >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  if (Number.isInteger(n)) return String(n);
  return n.toFixed(1);
}
