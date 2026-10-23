import type { MessagePart } from "../types";
import { isToolPart } from "./toolLabels";

type Entry = { part: MessagePart; msgIdx: number };

function parentId(part: MessagePart): string | null {
  const meta = part.state?.metadata;
  const id = part.parentCallID ?? meta?.parentCallID ?? meta?.parentToolCallId ?? meta?.parent_tool_call_id;
  return typeof id === "string" && id ? id : null;
}

/** Move only explicitly linked calls below their real parent. Orphans stay visible. */
export function nestToolParts(entries: Entry[]): Entry[] {
  if (!entries.some(({ part }) => isToolPart(part) && parentId(part))) return entries;
  const byId = new Map<string, number>();
  entries.forEach(({ part }, index) => {
    if (!isToolPart(part)) return;
    for (const id of [part.callID, part.toolCallId, part.id]) if (id) byId.set(id, index);
  });
  const parents = entries.map(({ part }) => {
    const id = isToolPart(part) ? parentId(part) : null;
    return id ? byId.get(id) : undefined;
  });
  const children = new Map<number, number[]>();
  parents.forEach((parent, index) => {
    if (parent === undefined) return;
    let current: number | undefined = parent;
    let depth = 0;
    while (current !== undefined && current !== index && depth < 32) {
      current = parents[current];
      depth++;
    }
    // Cycles and excessive nesting remain flat so no calls disappear or recurse forever.
    if (current !== undefined) {
      parents[index] = undefined;
      return;
    }
    const list = children.get(parent) ?? [];
    list.push(index);
    children.set(parent, list);
  });
  if (children.size === 0) return entries;
  function nested(index: number): MessagePart {
    const part = entries[index].part;
    const owned = children.get(index);
    return owned ? { ...part, toolChildren: owned.map(nested) } : part;
  }
  return entries.flatMap((entry, index) => {
    if (parents[index] !== undefined) return [];
    return [children.has(index) ? { ...entry, part: nested(index) } : entry];
  });
}
