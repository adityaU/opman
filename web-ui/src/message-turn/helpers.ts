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

/** Marker the server writes above a session's opening instructions. */
const INSTRUCTIONS_MARKER = "[Session instructions]";
/** Marker used before the rename — still present in older transcripts. */
const LEGACY_MARKER = "[Assistant memory in effect]";

/** Parsed session-instructions block from a user message. */
export interface MemoryBlock {
  items: { label: string; content: string }[];
  userText: string;
}

/**
 * Extract the session-instructions block from user message text.
 *
 * Both markers are accepted: the block is written by the server now, but
 * transcripts recorded before the rename still carry the old one.
 */
export function parseMemoryBlock(text: string): MemoryBlock | null {
  const marker = text.startsWith(INSTRUCTIONS_MARKER)
    ? INSTRUCTIONS_MARKER
    : text.startsWith(LEGACY_MARKER)
      ? LEGACY_MARKER
      : null;
  if (!marker) return null;

  const requestIdx = text.indexOf("[User request]");
  if (requestIdx === -1) return null;

  const memorySection = text.slice(marker.length, requestIdx).trim();
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
