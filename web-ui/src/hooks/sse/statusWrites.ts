import { SESSION_IDLE, type SessionStatus } from "./types";

/**
 * Whether two statuses mean the same thing to the UI.
 *
 * Retry carries its attempt and its next-try clock, so two retries are only
 * the same status while both of those hold; everything else is its `type`.
 */
export function sameStatus(a: SessionStatus, b: SessionStatus): boolean {
  if (a.type !== b.type) return false;
  if (a.type !== "retry" || b.type !== "retry") return true;
  return a.attempt === b.attempt && a.next === b.next;
}

/**
 * A status map with one session rewritten, or the same map when nothing moved.
 *
 * Idle is stored as absence: every reader treats a missing entry as idle, so
 * keeping `{type:"idle"}` around would give the same session two spellings.
 */
export function withStatus(
  statuses: Readonly<Record<string, SessionStatus>>,
  sessionId: string,
  status: SessionStatus,
): Readonly<Record<string, SessionStatus>> {
  const current = statuses[sessionId] ?? SESSION_IDLE;
  if (sameStatus(current, status)) return statuses;
  const next = { ...statuses };
  if (status.type === "idle") delete next[sessionId];
  else next[sessionId] = status;
  return next;
}

/** The busy set with one session added or removed, or the same set unchanged. */
export function withBusy(
  busy: Set<string>,
  sessionId: string,
  isBusy: boolean,
): Set<string> {
  if (isBusy === busy.has(sessionId)) return busy;
  const next = new Set(busy);
  if (isBusy) next.add(sessionId);
  else next.delete(sessionId);
  return next;
}
