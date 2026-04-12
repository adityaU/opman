import type { ModelRef } from "./types";

/** Render a model reference as a display string. */
export function modelLabel(m: ModelRef): string {
  if (typeof m === "string") return m;
  return m.modelID || JSON.stringify(m);
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
