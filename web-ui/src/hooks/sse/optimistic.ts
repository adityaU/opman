import type { Message } from "../../types";
import { getMessageTime, type MessageMap } from "./messageMap";

/**
 * Optimistic placeholders: locally-created user messages shown the instant a
 * prompt is submitted, before the runner has written it to the transcript.
 * They are keyed by an `__optimistic__` prefix so they can be told apart from
 * server records and retired once the real message arrives.
 */
const OPTIMISTIC_PREFIX = "__optimistic__";

/** Key for a new placeholder. Time-ordered so it sorts after existing records. */
export function createOptimisticId(): string {
  return `${OPTIMISTIC_PREFIX}${Date.now()}`;
}

export function isOptimisticId(id: string): boolean {
  return id.startsWith(OPTIMISTIC_PREFIX);
}

/** Concatenated text of a message, used to pair a placeholder with its
 *  server-confirmed twin. */
function messageText(msg: Message): string {
  let text = "";
  for (const part of msg.parts) {
    if (part.type === "text" && part.text) text += part.text;
  }
  return text.trim();
}

/** Remove all optimistic (not-yet-confirmed) messages from the map. */
export function purgeOptimistic(map: MessageMap): boolean {
  let removed = false;
  for (const key of map.keys()) {
    if (isOptimisticId(key)) { map.delete(key); removed = true; }
  }
  return removed;
}

/**
 * Keep only the optimistic placeholders belonging to `sessionId`.
 *
 * Switching into a session normally starts from a clean map, but a session
 * created by its own first send has no transcript yet — some runners (`claude
 * --bg`) write it seconds later. Dropping the placeholder there blanks the
 * message the user just sent and leaves the new-session empty state on screen.
 */
export function retainOptimistic(map: MessageMap, sessionId: string | null): MessageMap {
  const kept: MessageMap = new Map();
  if (!sessionId) return kept;
  for (const [key, msg] of map) {
    if (!isOptimisticId(key)) continue;
    if (msg.info.sessionID !== sessionId) continue;
    kept.set(key, msg);
  }
  return kept;
}

/**
 * Drop the placeholders the server has confirmed, leaving any that are still
 * only local (a queued follow-up, say). A placeholder counts as confirmed when a
 * real user message carries the same text, or when a real user message is newer
 * than it — the latter covers runners that reformat the prompt on write, the
 * former covers a server clock running behind the browser's.
 */
export function reconcileOptimistic(map: MessageMap): boolean {
  const confirmedText = new Set<string>();
  let newestConfirmed = 0;
  for (const [key, msg] of map) {
    if (isOptimisticId(key) || msg.info.role !== "user") continue;
    confirmedText.add(messageText(msg));
    newestConfirmed = Math.max(newestConfirmed, getMessageTime(msg));
  }
  if (confirmedText.size === 0) return false;

  let removed = false;
  for (const [key, msg] of map) {
    if (!isOptimisticId(key)) continue;
    if (!confirmedText.has(messageText(msg)) && getMessageTime(msg) > newestConfirmed) continue;
    map.delete(key);
    removed = true;
  }
  return removed;
}
