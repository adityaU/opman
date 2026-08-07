import type { Message } from "../../types";
import { getMessageTime, type MessageMap } from "./messageMap";

/**
 * Optimistic placeholders: locally-created user messages shown the instant a
 * prompt is submitted, before the runner has written it to the transcript.
 * They are keyed by an `__optimistic__` prefix so they can be told apart from
 * server records and retired once the real message arrives.
 */
const OPTIMISTIC_PREFIX = "__optimistic__";

/** Distinguishes placeholders created within the same millisecond, which would
 *  otherwise share a key and silently overwrite each other. */
let optimisticSeq = 0;

/** Key for a new placeholder. Time-ordered so it sorts after existing records. */
export function createOptimisticId(): string {
  optimisticSeq = (optimisticSeq + 1) % 1000;
  return `${OPTIMISTIC_PREFIX}${Date.now()}-${optimisticSeq}`;
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
 * Placeholders for sessions that are not on screen.
 *
 * A send can target a session other than the displayed one — a lazily created
 * session, or one the user navigated away from while the request was in flight.
 * Writing the placeholder into the live map then put the message in the *wrong*
 * conversation, permanently: the live map is the object the displayed session's
 * LRU entry was stored by reference, so restoring that session later brought
 * the foreign turn back with it.
 */
export type OptimisticStash = Map<string, MessageMap>;

/** Park a placeholder until its session is hydrated. */
export function stashOptimistic(
  stash: OptimisticStash,
  sessionId: string,
  id: string,
  msg: Message,
): void {
  const existing = stash.get(sessionId);
  if (existing) {
    existing.set(id, msg);
    return;
  }
  stash.set(sessionId, new Map([[id, msg]]));
}

/** Remove and return the placeholders parked for `sessionId`. */
export function takeOptimistic(stash: OptimisticStash, sessionId: string): MessageMap {
  const parked = stash.get(sessionId);
  if (!parked) return new Map();
  stash.delete(sessionId);
  return parked;
}

/** Forget a parked placeholder — a send that failed before it was ever shown. */
export function dropStashedOptimistic(stash: OptimisticStash, sessionId: string, id: string): void {
  const parked = stash.get(sessionId);
  if (!parked) return;
  parked.delete(id);
  if (parked.size === 0) stash.delete(sessionId);
}

/**
 * Drop placeholders that belong to a different session.
 *
 * Restoring a cached transcript is the one path that adopts a map wholesale, so
 * it is also the one that would resurrect a stray placeholder written into it by
 * an earlier bug or a race. Filtering here keeps a map self-consistent with the
 * session it is being restored for.
 */
export function purgeForeignOptimistic(map: MessageMap, sessionId: string): boolean {
  let removed = false;
  for (const [key, msg] of map) {
    if (!isOptimisticId(key)) continue;
    if (msg.info.sessionID === sessionId) continue;
    map.delete(key);
    removed = true;
  }
  return removed;
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
    // A message whose parts have not arrived yet is no evidence at all. Its envelope
    // lands one event before its text, and counting it here decided the match on
    // timestamps alone — which compares the *browser's* clock against the server's. A
    // phone running a few seconds fast kept the placeholder forever: the prompt showed
    // twice, and because the stale copy sorted newest the transcript also reported "No
    // response" even though the reply had arrived.
    const text = messageText(msg);
    if (!text) continue;
    confirmedText.add(text);
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
