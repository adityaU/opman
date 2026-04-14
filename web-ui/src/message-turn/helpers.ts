import type { ModelRef } from "./types";

/** Render a model reference as a display string. */
export function modelLabel(m: ModelRef): string {
  if (typeof m === "string") return m;
  return m.modelID || JSON.stringify(m);
}

// ── File context parsing ────────────────────────────────────────

/** Parsed file context from a user message with @file mentions. */
export interface FileContextBlock {
  /** File paths that were attached. */
  paths: string[];
  /** The user's actual text with all <file> blocks stripped. */
  userText: string;
}

const FILE_TAG_RE = /<file\s+path="([^"]+)">[^]*?<\/file>/g;

/**
 * Strip `<file path="...">...</file>` blocks injected by @file mentions.
 * Returns null if no file blocks are present.
 */
export function parseFileContext(text: string): FileContextBlock | null {
  if (!text.includes("<file path=")) return null;

  const paths: string[] = [];
  for (const m of text.matchAll(FILE_TAG_RE)) {
    paths.push(m[1]);
  }
  if (paths.length === 0) return null;

  const userText = text.replace(FILE_TAG_RE, "").trim();
  return { paths, userText };
}

/** Parsed memory block from a user message. */
export interface MemoryBlock {
  items: { label: string; content: string }[];
  userText: string;
}

/**
 * Extract `[Assistant memory in effect]` block from user message text.
 * Returns null if no memory block is present.
 */
export function parseMemoryBlock(text: string): MemoryBlock | null {
  if (!text.startsWith("[Assistant memory in effect]")) return null;

  const requestIdx = text.indexOf("[User request]");
  if (requestIdx === -1) return null;

  const memorySection = text.slice("[Assistant memory in effect]".length, requestIdx).trim();
  const userText = text.slice(requestIdx + "[User request]".length).trim();

  const items: MemoryBlock["items"] = [];
  for (const line of memorySection.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("- ")) continue;
    const colonIdx = trimmed.indexOf(":", 2);
    if (colonIdx === -1) {
      items.push({ label: trimmed.slice(2).trim(), content: "" });
      continue;
    }
    items.push({
      label: trimmed.slice(2, colonIdx).trim(),
      content: trimmed.slice(colonIdx + 1).trim(),
    });
  }

  if (items.length === 0) return null;
  return { items, userText };
}
