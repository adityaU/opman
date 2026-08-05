import type { Message, MessagePart } from "../../types";

// ── Helpers for extracting message ID ──────────────────────────────

/** Get the canonical message ID from a Message's info field.
 *  REST responses use `messageID`, SSE events use `id`. */
export function getMessageId(msg: Message): string {
  return msg.info.messageID || msg.info.id || "";
}

/** Get the creation timestamp from a message for sorting. */
export function getMessageTime(msg: Message): number {
  const t = msg.info.time;
  if (typeof t === "number") return t;
  if (t && typeof t === "object") return t.created ?? 0;
  // Also check metadata.time which REST responses include
  if (msg.metadata?.time?.created) return msg.metadata.time.created;
  return 0;
}

// ── Message Map management ─────────────────────────────────────────

/**
 * In-memory message store keyed by message ID.
 * This allows O(1) upserts from SSE events instead of re-fetching all messages.
 */
export type MessageMap = Map<string, Message>;

/** Merge a server history message into a cached/live message without losing parts. */
export function mergeMessage(existing: Message, incoming: Message): Message {
  const parts = new Map<string, MessagePart>();
  for (const part of existing.parts) {
    parts.set(part.id || `${part.type}-${parts.size}`, part);
  }
  for (const part of incoming.parts) {
    parts.set(part.id || `${part.type}-${parts.size}`, part);
  }
  return {
    ...existing,
    info: { ...existing.info, ...incoming.info },
    metadata: incoming.metadata ?? existing.metadata,
    parts: Array.from(parts.values()),
  };
}

/** Build a conservative fingerprint for duplicate records returned by a runner. */
function duplicateFingerprint(msg: Message): string | null {
  const time = getMessageTime(msg);
  if (!time || msg.parts.length === 0) return null;
  const parts = msg.parts.map((part) => ({
    type: part.type,
    text: part.text ?? null,
    tool: part.tool ?? null,
    callID: part.callID ?? part.toolCallId ?? null,
    state: part.state ?? null,
  }));
  return JSON.stringify([msg.info.role, time, parts]);
}

/** Convert a MessageMap to a sorted array for rendering. */
export function mapToSortedArray(map: MessageMap): Message[] {
  const seen = new Set<string>();
  return Array.from(map.values())
    .sort((a, b) => getMessageTime(a) - getMessageTime(b))
    .filter((message) => {
      const fingerprint = duplicateFingerprint(message);
      if (!fingerprint) return true;
      if (seen.has(fingerprint)) return false;
      seen.add(fingerprint);
      return true;
    });
}

/* Optimistic placeholder handling lives in ./optimistic.ts */

/** Upsert message info from a `message.updated` SSE event.
 *  Merges new info fields into the existing message if it exists,
 *  preserving the parts array.  Creates a new message entry if not. */
export function upsertMessageInfo(map: MessageMap, info: Record<string, unknown>): boolean {
  // SSE events use `id`, REST uses `messageID`
  const msgId = (info.id as string) || (info.messageID as string);
  if (!msgId) return false;

  const existing = map.get(msgId);
  if (existing) {
    // Merge info fields — keep existing parts
    const merged: Message = {
      ...existing,
      info: { ...existing.info, ...info, messageID: msgId },
      metadata: {
        ...existing.metadata,
        cost: (info.cost as number) ?? existing.metadata?.cost,
        time: info.time
          ? typeof info.time === "object"
            ? (info.time as { created?: number; completed?: number })
            : { created: info.time as number }
          : existing.metadata?.time,
        tokens: info.tokens
          ? (() => {
              const t = info.tokens as Record<string, unknown>;
              const cache = t.cache as Record<string, number> | undefined;
              return {
                input: (t.input as number) ?? 0,
                output: (t.output as number) ?? 0,
                reasoning: (t.reasoning as number) ?? 0,
                cache_read: cache?.read ?? 0,
                cache_write: cache?.write ?? 0,
              };
            })()
          : existing.metadata?.tokens,
      },
    };
    map.set(msgId, merged);
    return true;
  }

  // New message — create with empty parts (parts come via message.part.updated)
  const role = (info.role as string) || "assistant";
  const newMsg: Message = {
    info: {
      ...(info as unknown as Message["info"]),
      role: role as Message["info"]["role"],
      messageID: msgId,
    },
    parts: [],
    metadata: {
      cost: (info.cost as number) ?? 0,
      time: info.time
        ? typeof info.time === "object"
          ? (info.time as { created?: number; completed?: number })
          : { created: info.time as number }
        : undefined,
      tokens: info.tokens
        ? (() => {
            const t = info.tokens as Record<string, unknown>;
            const cache = t.cache as Record<string, number> | undefined;
            return {
              input: (t.input as number) ?? 0,
              output: (t.output as number) ?? 0,
              reasoning: (t.reasoning as number) ?? 0,
              cache_read: cache?.read ?? 0,
              cache_write: cache?.write ?? 0,
            };
          })()
        : undefined,
    },
  };
  map.set(msgId, newMsg);
  return true;
}

/** Upsert a part from a `message.part.updated` SSE event.
 *  Finds the parent message and adds/replaces the part by its `id`. */
export function upsertPart(map: MessageMap, part: Record<string, unknown>): boolean {
  const msgId = part.messageID as string;
  const partId = part.id as string;
  if (!msgId || !partId) return false;

  let msg = map.get(msgId);
  if (!msg) {
    // The part arrived before the message.updated event — create a skeleton
    msg = {
      info: {
        role: "assistant",
        messageID: msgId,
        sessionID: part.sessionID as string,
      },
      parts: [],
    };
    map.set(msgId, msg);
  }

  // Find existing part by id and replace, or append
  const existingIdx = msg.parts.findIndex((p) => p.id === partId);
  const newPart = part as unknown as MessagePart;
  if (existingIdx >= 0) {
    msg.parts[existingIdx] = newPart;
  } else {
    msg.parts.push(newPart);
  }

  // Must create a new message object reference for React to detect change
  map.set(msgId, { ...msg, parts: [...msg.parts] });
  return true;
}

/** Apply a text delta from a `message.part.delta` SSE event.
 *  Appends the delta string to the specified field of the part. */
export function applyPartDelta(
  map: MessageMap,
  sessionID: string,
  messageID: string,
  partID: string,
  field: string,
  delta: string,
): boolean {
  const msg = map.get(messageID);
  if (!msg) return false;

  const part = msg.parts.find((p) => p.id === partID);
  if (!part) {
    // Part not yet in the message — create a placeholder text part
    const newPart: MessagePart = {
      type: "text",
      id: partID,
      sessionID,
      messageID,
      [field]: delta,
    };
    msg.parts.push(newPart);
    map.set(messageID, { ...msg, parts: [...msg.parts] });
    return true;
  }

  // Append delta to the field (usually "text")
  const current = (part as unknown as Record<string, unknown>)[field];
  (part as unknown as Record<string, unknown>)[field] =
    typeof current === "string" ? current + delta : delta;

  // Create new references for React change detection
  map.set(messageID, { ...msg, parts: [...msg.parts] });
  return true;
}

/** Remove a message from the map. */
export function removeMessage(map: MessageMap, messageID: string): boolean {
  return map.delete(messageID);
}

/** Remove a part from a message. */
export function removePart(map: MessageMap, messageID: string, partID: string): boolean {
  const msg = map.get(messageID);
  if (!msg) return false;

  const filtered = msg.parts.filter((p) => p.id !== partID);
  if (filtered.length === msg.parts.length) return false; // wasn't there

  map.set(messageID, { ...msg, parts: filtered });
  return true;
}
