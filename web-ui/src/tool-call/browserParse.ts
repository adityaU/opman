/**
 * Reading the browser tools' own output format.
 *
 * The tools answer in plain text on purpose — the whole feature exists to keep a
 * page read cheap, and wrapping it in JSON for this card's benefit would put
 * tokens back on every call the model makes. So the card parses the format
 * instead, and every parser here degrades to "show the text as it came" rather
 * than failing.
 */

export type BrowserAction =
  | "open"
  | "snapshot"
  | "read"
  | "click"
  | "type"
  | "key"
  | "scroll"
  | "navigate"
  | "screenshot"
  | "panes"
  | "other";

export function browserAction(toolName: string): BrowserAction {
  const name = toolName.toLowerCase();
  if (name.includes("list_panes")) return "panes";
  if (name.includes("screenshot")) return "screenshot";
  if (name.includes("read_text")) return "read";
  if (name.includes("snapshot")) return "snapshot";
  if (name.includes("press_key")) return "key";
  if (name.includes("scroll")) return "scroll";
  if (name.includes("navigate")) return "navigate";
  if (name.includes("click")) return "click";
  if (name.includes("type")) return "type";
  if (name.includes("open")) return "open";
  return "other";
}

export function isBrowserTool(toolName: string): boolean {
  const name = toolName.toLowerCase();
  return name.includes("browser_") || name.endsWith("browser");
}

/** A page result: title, URL, and either an outline or prose. */
export interface BrowserPageResult {
  readonly title: string;
  readonly url: string;
  readonly body: string;
  readonly truncated: boolean;
}

/**
 * `title \n url \n\n body`, with an optional truncation footer. Anything that
 * does not match that shape becomes a body with no page — which still renders.
 */
export function parsePageResult(output: string | null): BrowserPageResult | null {
  if (!output) return null;
  const truncated = /\[(outline )?truncated/i.test(output);
  const lines = output.split("\n");
  const blank = lines.indexOf("");

  if (blank < 1 || !looksLikeUrl(lines[blank - 1])) {
    return { title: "", url: "", body: output.trim(), truncated };
  }

  return {
    title: lines.slice(0, blank - 1).join(" ").trim(),
    url: lines[blank - 1].trim(),
    body: lines
      .slice(blank + 1)
      .join("\n")
      .replace(/\n*\[(outline )?truncated[^\]]*\]\s*$/i, "")
      .trimEnd(),
    truncated,
  };
}

function looksLikeUrl(value: string | undefined): boolean {
  return typeof value === "string" && /^(https?:\/\/|about:|file:)/.test(value.trim());
}

/** `Screenshot of <url> (base64 JPEG):\n<data>` */
export function parseScreenshot(output: string | null): { url: string; data: string } | null {
  if (!output) return null;
  const newline = output.indexOf("\n");
  if (newline < 0) return null;
  const header = output.slice(0, newline);
  const data = output.slice(newline + 1).trim();
  if (!header.toLowerCase().startsWith("screenshot of") || !data) return null;
  return { url: header.replace(/^screenshot of\s*/i, "").replace(/\(.*\)?:?$/, "").trim(), data };
}

export interface BrowserPaneRow {
  readonly project: string;
  readonly title: string;
  readonly url: string;
}

/** `project  title  url`, two spaces between. */
export function parsePanes(output: string | null): readonly BrowserPaneRow[] {
  if (!output) return [];
  return output
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("No browsers"))
    .map((line) => {
      const parts = line.split(/\s{2,}/);
      return {
        project: parts[0] ?? "",
        title: parts.length > 2 ? parts[1] : "",
        url: parts[parts.length - 1] ?? "",
      };
    })
    .filter((row) => row.url.length > 0);
}

/** The host, which is what identifies a page at a glance. */
export function hostOf(url: string): string {
  if (!url) return "";
  try {
    return new URL(url).host || url;
  } catch {
    return url.replace(/^\w+:\/\//, "").split("/")[0];
  }
}

/** The path, for the second line — empty for a bare host. */
export function pathOf(url: string): string {
  try {
    const parsed = new URL(url);
    const path = `${parsed.pathname}${parsed.search}`;
    return path === "/" ? "" : path;
  } catch {
    return "";
  }
}
